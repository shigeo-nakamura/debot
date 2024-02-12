// derivative_trader.rs

use debot_market_analyzer::MarketData;
use debot_market_analyzer::PricePoint;
use debot_market_analyzer::TradingStrategy;
use debot_position_manager::TradePosition;
use dex_connector::DexConnector;
use dex_connector::DexError;
use futures::future::join_all;
use futures::FutureExt;
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
    max_price_size: u32,
    initial_balance: f64,
    max_dd_ratio: f64,
    rest_endpoint: String,
    web_socket_endpoint: String,
}

struct DerivativeTraderState {
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    fund_manager_map: HashMap<String, FundManager>,
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
        max_price_size: u32,
        db_handler: Arc<Mutex<DBHandler>>,
        open_positions_map: HashMap<String, HashMap<u32, TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        max_dd_ratio: f64,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        rest_endpoint: &str,
        web_socket_endpoint: &str,
        leverage: f64,
        strategy: &TradingStrategy,
    ) -> Self {
        const SECONDS_IN_MINUTE: usize = 60;
        let mut config = DerivativeTraderConfig {
            trader_name: dex_name.to_owned(),
            dex_name: dex_name.to_owned(),
            dry_run,
            short_trade_period: sample_interval.short_term * SECONDS_IN_MINUTE,
            long_trade_period: sample_interval.long_term * SECONDS_IN_MINUTE,
            trade_period: trade_interval * SECONDS_IN_MINUTE,
            max_price_size: max_price_size,
            initial_balance: 0.0,
            max_dd_ratio,
            rest_endpoint: rest_endpoint.to_owned(),
            web_socket_endpoint: web_socket_endpoint.to_owned(),
        };

        let state = Self::initialize_state(
            &mut config,
            db_handler,
            open_positions_map,
            price_market_data,
            load_prices,
            save_prices,
            order_effective_duration_secs,
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
        open_positions_map: HashMap<String, HashMap<u32, TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        leverage: f64,
        strategy: &TradingStrategy,
    ) -> DerivativeTraderState {
        let dex_connector = Self::create_dex_connector(config)
            .await
            .expect("Failed to initialize DexConnector");

        let fund_managers = Self::create_fund_managers(
            config,
            db_handler.clone(),
            dex_connector.clone(),
            &open_positions_map,
            &price_market_data,
            load_prices,
            save_prices,
            order_effective_duration_secs,
            use_market_order,
            strategy,
        );

        let mut state = DerivativeTraderState {
            db_handler,
            dex_connector,
            fund_manager_map: HashMap::new(),
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
        open_positions_map: &HashMap<String, HashMap<u32, TradePosition>>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        strategy: &TradingStrategy,
    ) -> Vec<FundManager> {
        let fund_manager_configurations = fund_config::get(&config.dex_name, strategy);
        let mut token_name_indices = HashMap::new();

        fund_manager_configurations
            .into_iter()
            .map(
                |(token_name, initial_amount, position_size_ratio, risk_reward, loss_cut_ratio)| {
                    let fund_name = format!(
                        "{:?}-{}-{}-{}",
                        strategy,
                        token_name,
                        config.short_trade_period / 60,
                        config.long_trade_period / 60
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

                    let index = *token_name_indices.entry(token_name.clone()).or_insert(0);
                    *token_name_indices.get_mut(&token_name).unwrap() += 1;

                    log::info!("create {}-{}-{}", fund_name, index, token_name);

                    FundManager::new(
                        &fund_name,
                        index,
                        &token_name,
                        open_positions_map.get(&fund_name).cloned(),
                        market_data,
                        *strategy,
                        initial_amount * position_size_ratio,
                        initial_amount,
                        config.trade_period,
                        risk_reward,
                        db_handler.clone(),
                        dex_connector.clone(),
                        save_prices,
                        order_effective_duration_secs,
                        use_market_order,
                        loss_cut_ratio,
                    )
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
        }
    }

    fn create_market_data(config: DerivativeTraderConfig) -> MarketData {
        MarketData::new(
            config.trader_name.to_owned(),
            config.short_trade_period,
            config.long_trade_period,
            config.trade_period,
            config.max_price_size as usize,
        )
    }

    pub async fn is_max_dd_occurred(&self) -> Result<bool, ()> {
        let balance = match self.get_balance().await {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let lost = self.config.initial_balance - balance;
        if lost > 0.0 {
            let dd_ratio = lost / self.config.initial_balance;
            log::info!(
                "lost = {}, initial_balance = {}, dd_ratio = {}",
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

        let mut prices: HashMap<String, Option<f64>> = HashMap::new();
        for result in price_results {
            let (token_name, price) = result?;
            prices.insert(token_name.to_owned(), price);
        }

        // 2. Check newly filled orders after the new price is queried; otherwise DexEmulator can't fill any orders
        for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
            let filled_orders = self
                .state
                .dex_connector
                .get_filled_orders(fund_manager.token_name())
                .await?;

            for order in filled_orders.orders {
                fund_manager
                    .position_filled(
                        order.order_id.clone(),
                        order.filled_side.clone(),
                        order.filled_value.clone(),
                        order.filled_size.clone(),
                        order.filled_fee.clone(),
                    )
                    .await
                    .map_err(|_| Box::new(io::Error::new(ErrorKind::Other, "An error occurred")))?;
            }
        }

        // Find trade chanes
        let find_futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .filter_map(|fund_manager| {
                let token_name = fund_manager.token_name();
                if let Some(price) = prices.get(token_name).and_then(|p| *p) {
                    Some(fund_manager.find_chances(price))
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

        Ok(())
    }

    pub async fn reset_dex_client(&mut self) -> bool {
        log::info!("reset dex_client");

        if self.state.dex_connector.stop().await.is_err() {
            log::error!("Failed to stop the dex_connector");
        }

        self.state.dex_connector = match Self::create_dex_connector(&self.config).await {
            Ok(v) => v,
            Err(e) => {
                log::error!("{:?}", e);
                return false;
            }
        };

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

    pub async fn get_balance(&self) -> Result<f64, ()> {
        if let Ok(res) = self.state.dex_connector.get_balance().await {
            return Ok(res.equity);
        }
        log::error!("failed to get the balance");
        return Err(());
    }
}
