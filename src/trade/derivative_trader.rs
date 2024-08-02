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
    order_effective_duration_secs: i64,
    max_price_size: u32,
    initial_balance: Decimal,
    max_dd_ratio: Decimal,
    rest_endpoint: String,
    web_socket_endpoint: String,
    save_prices: bool,
}

struct DerivativeTraderState {
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    fund_manager_map: HashMap<String, FundManager>,
    market_data_map: Arc<RwLock<HashMap<(String, TradingStrategy), Arc<RwLock<MarketData>>>>>,
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
        interval_msecs: u64,
        max_price_size: u32,
        db_handler: Arc<Mutex<DBHandler>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        max_dd_ratio: Decimal,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        risk_reward: Decimal,
        rest_endpoint: &str,
        web_socket_endpoint: &str,
        leverage: u32,
        strategy: Option<&TradingStrategy>,
    ) -> Self {
        log::info!("DerivativeTrader::new");
        const SECONDS_IN_MINUTE: usize = 60;
        let interval_secs = interval_msecs as i64 / 1000;

        let mut config = DerivativeTraderConfig {
            trader_name: dex_name.to_owned(),
            dex_name: dex_name.to_owned(),
            dry_run,
            short_trade_period: sample_interval.short_term * SECONDS_IN_MINUTE
                / interval_secs as usize,
            long_trade_period: sample_interval.long_term * SECONDS_IN_MINUTE
                / interval_secs as usize,
            trade_period: trade_interval * SECONDS_IN_MINUTE / interval_secs as usize,
            order_effective_duration_secs,
            max_price_size: max_price_size,
            initial_balance: Decimal::new(0, 0),
            max_dd_ratio,
            rest_endpoint: rest_endpoint.to_owned(),
            web_socket_endpoint: web_socket_endpoint.to_owned(),
            save_prices,
        };

        let state = Self::initialize_state(
            &mut config,
            db_handler,
            price_market_data,
            load_prices,
            order_effective_duration_secs,
            use_market_order,
            risk_reward,
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
        order_effective_duration_secs: i64,
        use_market_order: bool,
        risk_reward: Decimal,
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
            order_effective_duration_secs,
            use_market_order,
            risk_reward,
            strategy,
            market_data_map.clone(),
        )
        .await;

        let mut state = DerivativeTraderState {
            db_handler,
            dex_connector,
            fund_manager_map: HashMap::new(),
            market_data_map,
        };

        for fund_manager in fund_managers {
            fund_manager.initialize(leverage).await;
            state
                .fund_manager_map
                .insert(fund_manager.fund_name().to_owned(), fund_manager);
        }

        state
    }

    async fn create_fund_managers(
        config: &mut DerivativeTraderConfig,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_connector: Arc<DexConnectorBox>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        risk_reward: Decimal,
        strategy: Option<&TradingStrategy>,
        market_data_map: Arc<RwLock<HashMap<(String, TradingStrategy), Arc<RwLock<MarketData>>>>>,
    ) -> Vec<FundManager> {
        log::info!("DerivativeTrader::create_fund_managers");
        let fund_manager_configurations = fund_config::get(&config.dex_name, strategy);
        let mut token_name_indices = HashMap::new();
        let mut futures = vec![];

        for (
            token_name,
            strategy,
            initial_amount,
            position_size_ratio,
            take_profit_ratio,
            atr_spread,
            max_open_hours,
        ) in fund_manager_configurations.into_iter()
        {
            let db_handler = db_handler.clone();
            let dex_connector = dex_connector.clone();
            let config = config.clone();
            let price_market_data = price_market_data.clone();
            let load_prices = load_prices;
            let order_effective_duration_secs = order_effective_duration_secs;
            let use_market_order = use_market_order;
            let risk_reward = risk_reward;
            let index = *token_name_indices.entry(token_name.clone()).or_insert(0);
            *token_name_indices.get_mut(&token_name).unwrap() += 1;

            let fund_name = format!(
                "{}-{:?}-{}-{}-{:?}-{:?}",
                if config.dry_run { "test" } else { "prod" },
                strategy,
                token_name,
                index,
                take_profit_ratio,
                atr_spread,
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

                        if load_prices {
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
                    order_effective_duration_secs,
                    max_open_hours * 60 * 60,
                    use_market_order,
                    take_profit_ratio,
                    risk_reward,
                    atr_spread,
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
            for price_point in price_points {
                market_data.add_price(Some(price_point.price), Some(price_point.timestamp));
            }
        }
        log::info!("restore_market_data return");
    }

    async fn create_market_data(
        db_handler: Arc<Mutex<DBHandler>>,
        config: DerivativeTraderConfig,
        token_name: &str,
        strategy: &TradingStrategy,
    ) -> MarketData {
        let random_foreset = match strategy {
            TradingStrategy::MachineLearning(trend_type) => {
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
            config.order_effective_duration_secs,
            random_foreset,
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
        log::info!("1. Get token prices: started");

        let mut token_set = HashSet::new();
        let mut price_futures = Vec::new();

        for fund_manager in self.state.fund_manager_map.values_mut() {
            let token_name = fund_manager.token_name().to_owned();
            if !token_set.contains(&token_name) {
                token_set.insert(token_name.to_owned());
                price_futures.push(async move {
                    fund_manager
                        .get_token_price()
                        .await
                        .map(|price| (token_name, Some(price)))
                });
            }
        }

        let price_results = join_all(price_futures).await;
        log::info!("1. Get token prices: completed");

        let mut prices: HashMap<String, Option<(Decimal, Decimal)>> = HashMap::new();
        for result in price_results {
            let (token_name, price_min_tick) = result?;
            prices.insert(token_name.to_owned(), price_min_tick);
        }
        log::info!("Prices obtained: {:?}", prices);

        let mut saved_tokens = HashSet::new();
        let market_data_keys: Vec<_> = {
            log::info!("Acquiring read lock for market_data_map...");
            let market_data_map = self.state.market_data_map.read().await;
            log::info!("Read lock acquired for market_data_map");
            market_data_map.keys().cloned().collect()
        };
        log::info!("Market data keys obtained: {:?}", market_data_keys);

        for key in market_data_keys {
            let token_name = &key.0;
            log::info!("Processing market data key: {:?}", key);
            if let Some((price, min_tick)) = prices.get(token_name).and_then(|p| *p) {
                let rounded_price = Self::round_price(price, Some(min_tick));
                log::info!("Rounded price for {}: {:.5}", token_name, rounded_price);

                let market_data_clone = {
                    log::info!("Acquiring read lock for market_data_map...");
                    let market_data_map = self.state.market_data_map.read().await;
                    log::info!("Read lock acquired for market_data_map");
                    market_data_map.get(&key).cloned().unwrap()
                };
                log::info!("Market data clone obtained for key: {:?}", key);

                let price_point =
                    match timeout(Duration::from_secs(5), market_data_clone.write()).await {
                        Ok(mut market_data) => {
                            log::info!("Write lock acquired for market_data_clone");
                            market_data.add_price(Some(rounded_price), None)
                        }
                        Err(_) => {
                            log::error!(
                                "Timeout while trying to acquire write lock for market data: {:?}",
                                key
                            );
                            continue;
                        }
                    };
                log::info!("Price point added for token: {}", token_name);

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
                            log::info!("Lock acquired for db_handler");
                            db_handler.log_price(&key.0, token_name, price_point).await;
                        }
                        Err(_) => {
                            log::error!("Timeout while trying to acquire lock for DBHandler");
                            continue;
                        }
                    }
                    log::info!("Price logged for token: {}", token_name);

                    saved_tokens.insert(token_name.clone());
                }
            }
        }
        log::info!("All market data processed.");

        // 2. Check newly filled orders after the new price is queried; otherwise DexEmulator can't fill any orders
        log::info!("2. Check filled orders: started");
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
        log::info!("3. Check filled orders: finished");

        // 3. Find trade chanes
        let find_futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .filter_map(|fund_manager| {
                let token_name = fund_manager.token_name();
                if let Some((price, _min_tick)) = prices.get(token_name).and_then(|p| *p) {
                    Some(fund_manager.find_chances(price))
                } else {
                    None
                }
            })
            .collect();

        log::info!("3. Find trade chanes: started");
        let find_results = join_all(find_futures).await;
        log::info!("3. Find trade chanes: finished");

        for result in find_results {
            if result.is_err() {
                return result;
            }
        }

        // 6. Clean up the canceled positions
        for fund_manager in self.state.fund_manager_map.values_mut() {
            fund_manager.clean_canceled_position();
        }

        Ok(())
    }

    pub async fn reset_dex_client(&mut self) -> bool {
        log::info!("reset dex_client");

        if self.state.dex_connector.stop().await.is_err() {
            log::error!("Failed to stop the dex_connector");
        }

        if self.state.dex_connector.start().await.is_err() {
            log::error!("Failed to restart the dex_connector");
            return false;
        }

        for fund_manager in self.state.fund_manager_map.iter_mut() {
            fund_manager
                .1
                .reset_dex_client(self.state.dex_connector.clone());
        }

        true
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
            for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
                fund_manager.liquidate(Some(reason.to_owned())).await;
            }
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
}
