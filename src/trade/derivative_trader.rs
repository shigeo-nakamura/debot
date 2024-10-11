// derivative_trader.rs

use super::dex_connector_box::DexConnectorBox;
use super::fund_config;
use super::DBHandler;
use super::FundManager;
use debot_db::PricePoint;
use debot_market_analyzer::MarketData;
use debot_market_analyzer::TradingStrategy;
use dex_connector::DexConnector;
use dex_connector::DexError;
use dex_connector::FilledOrder;
use futures::future::join_all;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task;
use tokio::time::{timeout, Duration};

#[derive(Clone)]
pub struct SampleInterval {
    short_term: usize,
    long_term: usize,
}

impl SampleInterval {
    pub fn new(short_term: usize, long_term: usize) -> Self {
        Self {
            short_term,
            long_term,
        }
    }
}

#[derive(Clone)]
struct DerivativeTraderConfig {
    trader_name: String,
    dex_name: String,
    dry_run: bool,
    short_trade_period: usize,
    long_trade_period: usize,
    trade_period: usize,
    max_price_size: u32,
    initial_balance: Decimal,
    max_dd_ratio: Decimal,
    rest_endpoint: String,
    web_socket_endpoint: String,
    save_prices: bool,
    only_read_price: bool,
    back_test: bool,
    interval_secs: i64,
    input_data_chunk_size: Option<usize>,
}

struct DerivativeTraderState {
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    fund_manager_map: HashMap<String, FundManager>,
    market_data_map: Arc<RwLock<HashMap<(String, TradingStrategy), Arc<RwLock<MarketData>>>>>,
    back_test_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
    back_test_counter: usize,
}

pub struct DerivativeTrader {
    config: DerivativeTraderConfig,
    state: DerivativeTraderState,
}

impl DerivativeTrader {
    pub async fn new(
        dex_name: &str,
        dry_run: bool,
        trade_interval: usize,
        sample_interval: SampleInterval,
        interval_secs: i64,
        max_price_size: u32,
        db_handler: Arc<Mutex<DBHandler>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        max_dd_ratio: Decimal,
        close_order_effective_duration_secs: i64,
        use_market_order: bool,
        rest_endpoint: &str,
        web_socket_endpoint: &str,
        leverage: u32,
        strategy: Option<&TradingStrategy>,
        only_read_price: bool,
        back_test: bool,
        input_data_chunk_size: Option<usize>,
    ) -> Self {
        log::info!("DerivativeTrader::new");
        const SECONDS_IN_MINUTE: usize = 60;

        let mut config = DerivativeTraderConfig {
            trader_name: dex_name.to_owned(),
            dex_name: dex_name.to_owned(),
            dry_run,
            short_trade_period: sample_interval.short_term * SECONDS_IN_MINUTE
                / interval_secs as usize,
            long_trade_period: sample_interval.long_term * SECONDS_IN_MINUTE
                / interval_secs as usize,
            trade_period: trade_interval * SECONDS_IN_MINUTE / interval_secs as usize,
            max_price_size: max_price_size,
            initial_balance: Decimal::new(0, 0),
            max_dd_ratio,
            rest_endpoint: rest_endpoint.to_owned(),
            web_socket_endpoint: web_socket_endpoint.to_owned(),
            save_prices,
            only_read_price,
            back_test,
            interval_secs,
            input_data_chunk_size,
        };

        let state = Self::initialize_state(
            &mut config,
            db_handler,
            price_market_data,
            load_prices,
            close_order_effective_duration_secs,
            use_market_order,
            leverage,
            strategy,
        )
        .await;

        let mut this = Self { config, state };

        let balance = this.get_balance().await.unwrap();
        this.config.initial_balance = balance;

        this
    }

    async fn initialize_state(
        config: &mut DerivativeTraderConfig,
        db_handler: Arc<Mutex<DBHandler>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        close_order_effective_duration_secs: i64,
        use_market_order: bool,
        leverage: u32,
        strategy: Option<&TradingStrategy>,
    ) -> DerivativeTraderState {
        log::info!("DerivativeTrader::initialize_state");
        let dex_connector = Self::create_dex_connector(config)
            .await
            .expect("Failed to initialize DexConnector");

        let market_data_map = Arc::new(RwLock::new(HashMap::new()));

        let fund_managers = Self::create_fund_managers(
            config,
            db_handler.clone(),
            dex_connector.clone(),
            &price_market_data,
            load_prices,
            close_order_effective_duration_secs,
            use_market_order,
            strategy,
            market_data_map.clone(),
        )
        .await;

        let mut state = DerivativeTraderState {
            db_handler,
            dex_connector,
            fund_manager_map: HashMap::new(),
            market_data_map,
            back_test_data: if config.back_test {
                price_market_data
            } else {
                HashMap::new()
            },
            back_test_counter: 0,
        };

        log::info!("create_fund_managers() finished");

        let mut processed_tokens = HashSet::new();
        for fund_manager in fund_managers {
            let token_name = fund_manager.token_name();

            if !processed_tokens.contains(token_name) {
                if state
                    .dex_connector
                    .set_leverage(token_name, leverage)
                    .await
                    .is_err()
                {
                    panic!("Failed to set the leverage");
                }

                processed_tokens.insert(token_name.to_owned());
            }

            state
                .fund_manager_map
                .insert(fund_manager.fund_name().to_owned(), fund_manager);
        }

        log::info!("fund_manager.initialize() finished");

        state
    }

    async fn create_fund_managers(
        config: &mut DerivativeTraderConfig,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_connector: Arc<DexConnectorBox>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        close_order_effective_duration_secs: i64,
        use_market_order: bool,
        strategy: Option<&TradingStrategy>,
        market_data_map: Arc<RwLock<HashMap<(String, TradingStrategy), Arc<RwLock<MarketData>>>>>,
    ) -> Vec<FundManager> {
        log::info!("DerivativeTrader::create_fund_managers");
        let fund_manager_configurations =
            fund_config::get(&config.dex_name, strategy, config.interval_secs);
        let mut token_name_indices = HashMap::new();
        let mut futures = vec![];

        for (
            token_name,
            strategy,
            initial_amount,
            position_size_ratio,
            risk_reward,
            take_profit_ratio,
            atr_spread,
            atr_term,
            open_tick_count_max,
        ) in fund_manager_configurations.into_iter()
        {
            let db_handler = db_handler.clone();
            let dex_connector = dex_connector.clone();
            let config = config.clone();
            let price_market_data = price_market_data.clone();
            let load_prices = load_prices;
            let use_market_order = use_market_order;
            let risk_reward = risk_reward;

            let key = (token_name.clone(), strategy.clone());
            let index = *token_name_indices.entry(key.clone()).or_insert(0);
            *token_name_indices.get_mut(&key).unwrap() += 1;

            let fund_name = format!(
                "{}-{:?}-{}-{}-p/l({:?})-spread({:?})",
                if config.dry_run { "test" } else { "prod" },
                strategy,
                token_name,
                index,
                take_profit_ratio.unwrap_or_default(),
                atr_spread.unwrap_or_default(),
            );

            let market_data_key = (token_name.clone(), strategy.clone());
            let market_data_map = market_data_map.clone();

            let future = async move {
                let market_data = {
                    let mut map = market_data_map.write().await;
                    if let Some(market_data) = map.get(&market_data_key) {
                        market_data.clone()
                    } else {
                        let new_market_data = Arc::new(RwLock::new(
                            Self::create_market_data(
                                db_handler.clone(),
                                config.clone(),
                                &token_name,
                                &strategy,
                            )
                            .await,
                        ));

                        if !config.back_test && load_prices {
                            Self::restore_market_data(
                                new_market_data.clone(),
                                &config.trader_name,
                                &token_name,
                                &price_market_data,
                            )
                            .await;
                        }

                        map.insert(market_data_key.clone(), new_market_data.clone());
                        new_market_data
                    }
                };

                log::info!("create {}", fund_name);

                let open_order_tick_count_max = open_tick_count_max;
                let close_order_tick_count_max: u32 = (close_order_effective_duration_secs
                    / config.interval_secs)
                    .try_into()
                    .unwrap();

                let execution_delay_tick_count_max = open_tick_count_max;

                Some(FundManager::new(
                    &fund_name,
                    index,
                    &token_name,
                    market_data.clone(),
                    strategy,
                    initial_amount * position_size_ratio,
                    initial_amount,
                    db_handler,
                    dex_connector,
                    open_order_tick_count_max,
                    close_order_tick_count_max,
                    open_tick_count_max,
                    execution_delay_tick_count_max,
                    use_market_order,
                    take_profit_ratio,
                    risk_reward,
                    atr_spread,
                    atr_term,
                ))
            };

            futures.push(task::spawn(future));
        }

        let results = join_all(futures).await;
        results.into_iter().filter_map(|res| res.unwrap()).collect()
    }

    async fn create_dex_connector(
        config: &DerivativeTraderConfig,
    ) -> Result<Arc<DexConnectorBox>, DexError> {
        let dex_connector = DexConnectorBox::create(
            &config.dex_name,
            &config.rest_endpoint,
            &config.web_socket_endpoint,
            config.dry_run,
        )
        .await?;
        log::info!("create_dex_connector");
        dex_connector.start().await?;
        log::info!("dex_connector started");
        Ok(Arc::new(dex_connector))
    }

    async fn restore_market_data(
        market_data: Arc<RwLock<MarketData>>,
        trader_name: &str,
        token_name: &str,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
    ) {
        log::info!("restore_market_data enter: {}, {}", trader_name, token_name);
        let price_points = price_market_data
            .get(trader_name)
            .and_then(|price_points_map| price_points_map.get(token_name).cloned());

        if let Some(price_points) = price_points {
            let mut market_data = market_data.write().await;
            log::info!("num of data = {}", price_points.len());
            for price_point in price_points {
                market_data.add_price(
                    Some(price_point.price),
                    Some(price_point.timestamp),
                    price_point.volume,
                    price_point.num_trades,
                    price_point.funding_rate,
                    price_point.open_interest,
                    price_point.oracle_price,
                );
            }
        }
        log::info!("restore_market_data return");
    }

    fn get_back_test_price(
        trader_name: &str,
        token_name: &str,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        index: usize,
    ) -> Option<PricePoint> {
        if price_market_data.is_empty() {
            return None;
        }

        let price_points = price_market_data
            .get(trader_name)
            .and_then(|price_points_map| price_points_map.get(token_name).cloned());

        if price_points.is_none() {
            return None;
        }

        let price_points = price_points.unwrap();
        if price_points.len() <= index {
            return None;
        }

        let price_point = price_points[index].clone();

        log::debug!("back test data[{}] = {:?}", index, price_point);

        Some(price_point)
    }

    async fn create_market_data(
        db_handler: Arc<Mutex<DBHandler>>,
        config: DerivativeTraderConfig,
        token_name: &str,
        strategy: &TradingStrategy,
    ) -> MarketData {
        let random_foreset = match strategy {
            TradingStrategy::MeanReversion(trend_type) => {
                let position_type = match trend_type {
                    debot_market_analyzer::TrendType::Up => "Long",
                    debot_market_analyzer::TrendType::Down => "Short",
                    _ => "",
                };
                let key = format!("{}_{}", token_name, position_type);
                Some(db_handler.lock().await.create_random_forest(&key).await)
            }
            _ => None,
        };

        MarketData::new(
            config.trader_name.to_owned(),
            config.short_trade_period,
            config.long_trade_period,
            config.trade_period,
            config.max_price_size as usize,
            random_foreset,
            config.only_read_price,
        )
    }

    fn round_price(price: Decimal, min_tick: Option<Decimal>) -> Decimal {
        let min_tick = min_tick.unwrap_or(Decimal::ONE);
        (price / min_tick).round() * min_tick
    }

    pub async fn is_max_dd_occurred(&self) -> Result<bool, ()> {
        let balance = match self.get_balance().await {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let lost = self.config.initial_balance - balance;
        if lost.is_sign_positive() {
            let dd_ratio = lost / self.config.initial_balance;
            log::info!(
                "lost = {:.3}, initial_balance = {:.3}, dd_ratio = {:.3}",
                lost,
                self.config.initial_balance,
                dd_ratio
            );
            if dd_ratio > self.config.max_dd_ratio {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    pub async fn find_chances(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // 1. Get token prices
        log::debug!("1. Get token prices: started");

        let mut token_set = HashSet::new();
        let mut price_futures = Vec::new();

        for fund_manager in self.state.fund_manager_map.values_mut() {
            let token_name = fund_manager.token_name().to_owned();
            if !token_set.contains(&token_name) {
                token_set.insert(token_name.to_owned());
                let back_test_price = Self::get_back_test_price(
                    &self.config.trader_name,
                    &token_name,
                    &self.state.back_test_data,
                    self.state.back_test_counter,
                );
                if self.config.back_test && back_test_price.is_none() {
                    log::warn!(
                        "Back test is not available: counter = {}",
                        self.state.back_test_counter
                    );
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Back test is finished",
                    )));
                }

                price_futures.push(async move {
                    fund_manager
                        .get_token_price(back_test_price.as_ref())
                        .await
                        .map(|price| (token_name, Some(price)))
                });
            }
        }

        let price_results = join_all(price_futures).await;
        log::debug!("1. Get token prices: completed");

        let mut prices: HashMap<
            String,
            Option<(
                Decimal,
                Decimal,
                Option<i64>,
                Option<Decimal>,
                Option<u64>,
                Option<Decimal>,
                Option<Decimal>,
                Option<Decimal>,
            )>,
        > = HashMap::new();
        for result in price_results {
            let (token_name, price_point) = result?;
            prices.insert(token_name.to_owned(), price_point);
        }
        log::debug!("Prices obtained: {:?}", prices);

        self.state.back_test_counter += 1;

        let mut saved_tokens = HashSet::new();
        let market_data_keys: Vec<_> = {
            let market_data_map = self.state.market_data_map.read().await;
            market_data_map.keys().cloned().collect()
        };
        log::debug!("Market data keys obtained: {:?}", market_data_keys);

        for key in market_data_keys {
            let token_name = &key.0;
            log::debug!("Processing market data key: {:?}", key);
            if let Some((
                price,
                min_tick,
                timestamp,
                volume,
                num_trades,
                funding_rate,
                open_interest,
                oracle_price,
            )) = prices.get(token_name).and_then(|p| *p)
            {
                let rounded_price = Self::round_price(price, Some(min_tick));
                log::debug!("Rounded price for {}: {:.5}", token_name, rounded_price);

                let market_data_clone = {
                    let market_data_map = self.state.market_data_map.read().await;
                    market_data_map.get(&key).cloned().unwrap()
                };
                log::debug!("Market data clone obtained for key: {:?}", key);

                let price_point =
                    match timeout(Duration::from_secs(5), market_data_clone.write()).await {
                        Ok(mut market_data) => market_data.add_price(
                            Some(rounded_price),
                            timestamp,
                            volume,
                            num_trades,
                            funding_rate,
                            open_interest,
                            oracle_price,
                        ),
                        Err(_) => {
                            log::error!(
                                "Timeout while trying to acquire write lock for market data: {:?}",
                                key
                            );
                            continue;
                        }
                    };
                log::debug!("Price point added for token: {}", token_name);

                if self.config.save_prices && !saved_tokens.contains(token_name) {
                    log::trace!(
                        "{}: price = {:.5}, min_tick = {:.5?}, rounded_price = {:.5}",
                        token_name,
                        price,
                        min_tick,
                        price_point.price
                    );

                    match timeout(Duration::from_secs(5), self.state.db_handler.lock()).await {
                        Ok(db_handler) => {
                            db_handler
                                .log_price(&self.config.trader_name, token_name, price_point)
                                .await;
                        }
                        Err(_) => {
                            log::error!("Timeout while trying to acquire lock for DBHandler");
                            continue;
                        }
                    }
                    log::debug!("Price logged for token: {}", token_name);

                    saved_tokens.insert(token_name.clone());
                }

                if !self.config.only_read_price {
                    let strategy = key.1;
                    log::info!(
                        "Precompute the model for token: {}, strategy: {:?}",
                        token_name,
                        strategy
                    );

                    let (take_profit_ratio, atr_spread, open_tick_count_max, atr_term, count) =
                        fund_config::get_vectors_and_count(
                            &self.config.dex_name,
                            &strategy,
                            self.config.interval_secs,
                        );

                    match timeout(Duration::from_secs(5), market_data_clone.write()).await {
                        Ok(mut market_data) => {
                            market_data.precompute_models(
                                count,
                                &take_profit_ratio,
                                &atr_spread,
                                &open_tick_count_max,
                                &atr_term,
                                self.config.input_data_chunk_size,
                            );
                        }
                        Err(_) => {
                            log::error!(
                                "Timeout while trying to acquire write lock for market data: {:?}",
                                key
                            );
                            continue;
                        }
                    };
                }
            }
        }
        log::info!("All market data processed.");

        if self.config.only_read_price {
            return Ok(());
        }

        // 2. Check newly filled orders after the new price is queried; otherwise DexEmulator can't fill any orders
        log::debug!("2. Check filled orders: started");
        let mut filled_orders_map: HashMap<String, FilledOrder> = HashMap::new();
        for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
            let token_name = fund_manager.token_name();
            if filled_orders_map.get(token_name).is_none() {
                let filled_orders = self
                    .state
                    .dex_connector
                    .get_filled_orders(fund_manager.token_name())
                    .await?;
                for filled_order in filled_orders.orders {
                    filled_orders_map.insert(filled_order.trade_id.to_owned(), filled_order);
                }
            }
        }

        let mut filled_orders_map_clone = filled_orders_map.clone();

        for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
            for order in filled_orders_map.values() {
                if order.is_rejected {
                    fund_manager
                        .cancel_order(&order.order_id.clone(), true)
                        .await;
                } else {
                    let filled = fund_manager
                        .position_filled(
                            &order.order_id.clone(),
                            order.filled_side.clone().unwrap(),
                            order.filled_value.unwrap(),
                            order.filled_size.unwrap(),
                            order.filled_fee.unwrap(),
                        )
                        .await
                        .map_err(|_| {
                            Box::new(io::Error::new(ErrorKind::Other, "An error occurred"))
                        })?;
                    if filled {
                        fund_manager.clear_filled_order(&order.trade_id).await;
                        filled_orders_map_clone.remove(&order.trade_id);
                    }
                }
            }
        }
        self.state.dex_connector.clear_all_filled_order().await?;

        if !filled_orders_map_clone.is_empty() {
            log::warn!(
                "Some filled orders are not handled: {:?}",
                filled_orders_map_clone
            );
        }
        log::debug!("2. Check filled orders: finished");

        // 3. Find trade chanes
        let find_futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .filter_map(|fund_manager| {
                let token_name = fund_manager.token_name();
                if let Some((
                    price,
                    _min_tick,
                    _timestamp,
                    _volume,
                    _num_trades,
                    _funding_rate,
                    _open_interest,
                    _oracle_price,
                )) = prices.get(token_name).and_then(|p| *p)
                {
                    Some(fund_manager.find_chances(price, self.config.dry_run))
                } else {
                    None
                }
            })
            .collect();

        log::debug!("3. Find trade chances: started");
        let find_results = join_all(find_futures).await;
        log::debug!("3. Find trade chances: finished");

        for result in find_results {
            if result.is_err() {
                return result;
            }
        }

        // 4. Clean up the canceled positions
        for fund_manager in self.state.fund_manager_map.values_mut() {
            fund_manager.clean_canceled_position();
        }

        Ok(())
    }

    pub async fn reset_dex_client(&mut self) -> bool {
        log::info!("reset dex_client");

        let mut result = true;

        if self.state.dex_connector.restart().await.is_err() {
            log::error!("Failed to restart the dex_connector");
            result = false;
        }

        for fund_manager in self.state.fund_manager_map.iter_mut() {
            fund_manager
                .1
                .reset_dex_client(self.state.dex_connector.clone());
        }

        result
    }

    pub async fn liquidate(&mut self, on_exit: bool, reason: &str) {
        let res = self.state.dex_connector.cancel_all_orders(None).await;
        if let Err(e) = res {
            log::error!("liquidate failed (cancel): {:?}", e);
        }

        let res = self.state.dex_connector.close_all_positions(None).await;
        if let Err(e) = res {
            log::error!("liquidate failed (close position): {:?}", e);
        }

        if on_exit {
            let mut tasks = vec![];

            for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
                let reason = reason.to_owned();
                let task = async move {
                    fund_manager.liquidate(Some(reason)).await;
                };
                tasks.push(task);
            }

            join_all(tasks).await;
        }
    }

    pub fn db_handler(&self) -> &Arc<Mutex<DBHandler>> {
        &self.state.db_handler
    }

    pub async fn get_balance(&self) -> Result<Decimal, ()> {
        if let Ok(res) = self.state.dex_connector.get_balance().await {
            return Ok(res.equity);
        }
        log::error!("failed to get the balance");
        return Err(());
    }

    pub fn invested_amount(&self) -> Decimal {
        let mut sum = Decimal::ZERO;
        for (_, fund_manager) in self.state.fund_manager_map.iter() {
            sum += fund_manager.asset_in_usd();
        }
        sum.round_dp(1).abs()
    }
}
