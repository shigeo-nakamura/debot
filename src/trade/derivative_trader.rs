// derivative_trader.rs

use debot_db::PricePoint;
use debot_market_analyzer::MarketData;
use debot_market_analyzer::TradeAction;
use debot_market_analyzer::TradeDetail;
use debot_market_analyzer::TradingStrategy;
use debot_position_manager::ReasonForClose;
use debot_position_manager::State;
use dex_connector::DexConnector;
use dex_connector::DexError;
use dex_connector::FilledOrder;
use futures::future::join_all;
use futures::FutureExt;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::dex_connector_box::DexConnectorBox;
use super::fund_config;
use super::DBHandler;
use super::FundManager;

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
}

struct DerivativeTraderState {
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    fund_manager_map: HashMap<String, FundManager>,
    hedge_requests: Arc<Mutex<HashMap<String, TradeAction>>>,
    is_trend_changed: Arc<Mutex<HashMap<String, bool>>>,
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
        max_open_duration_secs: i64,
        use_market_order: bool,
        risk_reward: Decimal,
        rest_endpoint: &str,
        web_socket_endpoint: &str,
        leverage: u32,
        strategy: Option<&TradingStrategy>,
    ) -> Self {
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
        };

        let state = Self::initialize_state(
            &mut config,
            db_handler,
            price_market_data,
            load_prices,
            save_prices,
            order_effective_duration_secs,
            max_open_duration_secs,
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
        save_prices: bool,
        order_effective_duration_secs: i64,
        max_open_duration_secs: i64,
        use_market_order: bool,
        risk_reward: Decimal,
        leverage: u32,
        strategy: Option<&TradingStrategy>,
    ) -> DerivativeTraderState {
        let dex_connector = Self::create_dex_connector(config)
            .await
            .expect("Failed to initialize DexConnector");

        let fund_managers = Self::create_fund_managers(
            config,
            db_handler.clone(),
            dex_connector.clone(),
            &price_market_data,
            load_prices,
            save_prices,
            order_effective_duration_secs,
            max_open_duration_secs,
            use_market_order,
            risk_reward,
            strategy,
        );

        let mut state = DerivativeTraderState {
            db_handler,
            dex_connector,
            fund_manager_map: HashMap::new(),
            hedge_requests: Arc::new(Mutex::new(HashMap::new())),
            is_trend_changed: Arc::new(Mutex::new(HashMap::new())),
        };

        for fund_manager in fund_managers {
            fund_manager.initialize(leverage).await;
            state
                .fund_manager_map
                .insert(fund_manager.fund_name().to_owned(), fund_manager);
        }

        state
    }

    fn create_fund_managers(
        config: &mut DerivativeTraderConfig,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_connector: Arc<DexConnectorBox>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        order_effective_duration_secs: i64,
        max_open_duration_secs: i64,
        use_market_order: bool,
        risk_reward: Decimal,
        strategy: Option<&TradingStrategy>,
    ) -> Vec<FundManager> {
        let fund_manager_configurations = fund_config::get(&config.dex_name, strategy);
        let mut token_name_indices = HashMap::new();

        fund_manager_configurations
            .into_iter()
            .filter_map(
                |(
                    token_name,
                    pair_token_name,
                    strategy,
                    initial_amount,
                    position_size_ratio,
                    take_profit_ratio,
                )| {
                    let index = *token_name_indices.entry(token_name.clone()).or_insert(0);
                    *token_name_indices.get_mut(&token_name).unwrap() += 1;

                    let fund_name = format!(
                        "{}-{:?}-{}-{:3}-{})",
                        if config.dry_run { "test" } else { "prod" },
                        strategy,
                        token_name,
                        index,
                        take_profit_ratio,
                    );

                    let mut market_data = Self::create_market_data(config.clone());

                    if load_prices {
                        Self::restore_market_data(
                            &mut market_data,
                            &config.trader_name,
                            &token_name,
                            &price_market_data,
                        );
                    }

                    log::info!("create {}", fund_name);

                    Some(FundManager::new(
                        &fund_name,
                        index,
                        &token_name,
                        pair_token_name.as_deref(),
                        market_data,
                        strategy,
                        initial_amount * position_size_ratio,
                        initial_amount,
                        db_handler.clone(),
                        dex_connector.clone(),
                        save_prices,
                        order_effective_duration_secs,
                        max_open_duration_secs,
                        use_market_order,
                        take_profit_ratio,
                        risk_reward,
                    ))
                },
            )
            .collect()
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
        dex_connector.start().await?;
        Ok(Arc::new(dex_connector))
    }

    fn restore_market_data(
        market_data: &mut MarketData,
        trader_name: &str,
        token_name: &str,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
    ) {
        if let Some(price_points_map) = price_market_data.get(trader_name) {
            if let Some(price_points) = price_points_map.get(token_name) {
                for price_point in price_points {
                    market_data.add_price(Some(price_point.price), Some(price_point.timestamp));
                }
            }
            market_data.init_crossover_state();
        }
    }

    fn create_market_data(config: DerivativeTraderConfig) -> MarketData {
        MarketData::new(
            config.trader_name.to_owned(),
            config.short_trade_period,
            config.long_trade_period,
            config.trade_period,
            config.max_price_size as usize,
            config.order_effective_duration_secs,
        )
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
        let mut token_set = HashSet::new();
        let price_futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .filter_map(|fund_manager| {
                let token_name = fund_manager.token_name().to_owned();
                if token_set.contains(&token_name) {
                    None
                } else {
                    token_set.insert(token_name.to_owned());
                    let get_price = fund_manager.get_token_price();
                    Some(
                        async move { get_price.await.map(|price| (token_name, Some(price))) }
                            .boxed(),
                    )
                }
            })
            .collect();

        let price_results = join_all(price_futures).await;

        let mut prices: HashMap<String, Option<(Decimal, Decimal)>> = HashMap::new();
        for result in price_results {
            let (token_name, price_min_tick) = result?;
            prices.insert(token_name.to_owned(), price_min_tick);
        }

        // 2. Check newly filled orders after the new price is queried; otherwise DexEmulator can't fill any orders
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

        // 3. Find trade chanes
        let find_futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .filter_map(|fund_manager| {
                let token_name = fund_manager.token_name();
                if let Some((price, min_tick)) = prices.get(token_name).and_then(|p| *p) {
                    fund_manager.set_min_tick(min_tick);
                    Some(fund_manager.find_chances(
                        price,
                        self.state.hedge_requests.clone(),
                        self.state.is_trend_changed.clone(),
                    ))
                } else {
                    None
                }
            })
            .collect();

        let find_results = join_all(find_futures).await;

        for result in find_results {
            if result.is_err() {
                return result;
            }
        }

        // 4. Hedge positions
        let hedge_requests_lock = self.state.hedge_requests.lock().await;
        let hedge_requests = hedge_requests_lock.clone();
        drop(hedge_requests_lock);

        let hedge_futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .filter_map(|fund_manager| {
                let token_name = fund_manager.token_name();
                if let Some(hedge_action) = hedge_requests.get(token_name) {
                    Some(fund_manager.hedge_position(hedge_action.clone()))
                } else {
                    None
                }
            })
            .collect();

        let hedge_results = join_all(hedge_futures).await;

        self.state.hedge_requests.lock().await.clear();

        for result in hedge_results {
            if result.is_err() {
                return result;
            }
        }

        // 5. Do delta neutral
        self.do_delta_neutral().await?;

        // 6. Clean up the canceled positions
        for fund_manager in self.state.fund_manager_map.values_mut() {
            fund_manager.clean_canceled_position();
        }

        Ok(())
    }

    async fn do_delta_neutral(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut delta_map: HashMap<String, (Decimal, bool)> = HashMap::new();

        for (_, fund_manager) in &self.state.fund_manager_map {
            if let Some((delta_position, should_hedge_position)) = fund_manager.delta_position() {
                if let Some(pair_token_name) = fund_manager.pair_token_name() {
                    delta_map
                        .entry(pair_token_name.to_owned())
                        .or_insert((Decimal::ZERO, false));
                    delta_map.entry(pair_token_name.to_owned()).and_modify(|v| {
                        v.0 += delta_position;
                        v.1 = should_hedge_position;
                    });
                }
            }
        }

        let mut hedge_futures = vec![];
        for fund_manager in self.state.fund_manager_map.values_mut() {
            let token_name = fund_manager.token_name();
            if let TradingStrategy::PassiveTrade(hedge_ratio) = fund_manager.strategy() {
                if let Some((delta_position_amount, should_hedge_position)) =
                    delta_map.get(token_name)
                {
                    if !should_hedge_position {
                        continue;
                    }
                    let delta_position_amount = delta_position_amount * hedge_ratio;
                    let current_position_amount = match fund_manager.get_open_position() {
                        Some(v) => v.asset_in_usd(),
                        None => Decimal::ZERO,
                    };
                    let position_diff = delta_position_amount + current_position_amount;
                    if position_diff.abs() / (current_position_amount.abs() + Decimal::ONE)
                        < Decimal::new(1, 1)
                    {
                        continue;
                    }
                    self.state
                        .is_trend_changed
                        .lock()
                        .await
                        .insert(token_name.to_owned(), false);
                    let hedge_action = Self::create_hedge_action(position_diff);
                    hedge_futures.push(fund_manager.hedge_position(hedge_action));
                } else {
                    if let Some(is_trend_changed) =
                        self.state.is_trend_changed.lock().await.get(token_name)
                    {
                        if *is_trend_changed {
                            self.state
                                .is_trend_changed
                                .lock()
                                .await
                                .insert(token_name.to_owned(), false);
                            continue;
                        }
                    }
                    if let Some(hedge_position) = fund_manager.get_open_position() {
                        if matches!(hedge_position.state(), State::Open) {
                            fund_manager.cancel_all_orders().await;
                            let reason = ReasonForClose::Other("TrimHedgedPosition".to_owned());
                            fund_manager.close_open_position(Some(reason)).await;
                        }
                    }
                }
            }
        }

        let hedge_results = join_all(hedge_futures).await;

        for result in hedge_results {
            if result.is_err() {
                return result;
            }
        }

        Ok(())
    }

    fn create_hedge_action(amount_in_usd: Decimal) -> TradeAction {
        let confidence = Decimal::ONE;
        if amount_in_usd.is_sign_positive() {
            TradeAction::BuyHedge(TradeDetail::new(None, Some(amount_in_usd), confidence))
        } else {
            TradeAction::SellHedge(TradeDetail::new(None, Some(amount_in_usd), confidence))
        }
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

    pub async fn liquidate(&mut self, reason: &str) {
        for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
            fund_manager.liquidate(Some(reason.to_owned())).await;
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
