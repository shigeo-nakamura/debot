// derivative_trader.rs

use debot_market_analyzer::MarketData;
use debot_market_analyzer::PricePoint;
use debot_position_manager::TradePosition;
use dex_client::DexClient;
use futures::future::join_all;
use futures::FutureExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    short_trade_period: usize,
    long_trade_period: usize,
    trade_period: usize,
    max_price_size: u32,
    dex_router_api_key: String,
    dex_router_url: String,
    initial_balance: f64,
    max_dd_ratio: f64,
}

struct DerivativeTraderState {
    db_handler: Arc<Mutex<DBHandler>>,
    dex_client: DexClient,
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
        risk_reward: f64,
        db_handler: Arc<Mutex<DBHandler>>,
        open_positions_map: HashMap<String, Vec<TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        dex_router_api_key: &str,
        dex_router_url: &str,
        non_trading_period_secs: i64,
        positino_size_ratio: f64,
        max_dd_ratio: f64,
        order_effective_duration_secs: i64,
        use_market_order: bool,
    ) -> Self {
        const SECONDS_IN_MINUTE: usize = 60;
        let mut config = DerivativeTraderConfig {
            trader_name: dex_name.to_owned(),
            dex_name: dex_name.to_owned(),
            short_trade_period: sample_interval.short_term * SECONDS_IN_MINUTE,
            long_trade_period: sample_interval.long_term * SECONDS_IN_MINUTE,
            trade_period: trade_interval * SECONDS_IN_MINUTE,
            max_price_size: max_price_size,
            dex_router_api_key: dex_router_api_key.to_owned(),
            dex_router_url: dex_router_url.to_owned(),
            initial_balance: 0.0,
            max_dd_ratio,
        };

        let state = Self::initialize_state(
            &mut config,
            db_handler,
            dex_router_api_key,
            dex_router_url,
            open_positions_map,
            price_market_data,
            load_prices,
            save_prices,
            risk_reward,
            dry_run,
            non_trading_period_secs,
            positino_size_ratio,
            order_effective_duration_secs,
            use_market_order,
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
        dex_router_api_key: &str,
        dex_router_url: &str,
        open_positions_map: HashMap<String, Vec<TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        risk_reward: f64,
        dry_run: bool,
        non_trading_period_secs: i64,
        positino_size_ratio: f64,
        order_effective_duration_secs: i64,
        use_market_order: bool,
    ) -> DerivativeTraderState {
        let dex_client = Self::create_dex_clinet(dex_router_api_key, dex_router_url)
            .await
            .expect("Failed to initialize DexClient");

        let fund_managers = Self::create_fund_managers(
            config,
            db_handler.clone(),
            &dex_client,
            &open_positions_map,
            &price_market_data,
            load_prices,
            save_prices,
            risk_reward,
            dry_run,
            non_trading_period_secs,
            positino_size_ratio,
            order_effective_duration_secs,
            use_market_order,
        );

        let mut state = DerivativeTraderState {
            db_handler,
            dex_client,
            fund_manager_map: HashMap::new(),
        };

        for fund_manager in fund_managers {
            state
                .fund_manager_map
                .insert(fund_manager.fund_name().to_owned(), fund_manager);
        }

        state
    }

    fn create_fund_managers(
        config: &mut DerivativeTraderConfig,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_client: &DexClient,
        open_positions_map: &HashMap<String, Vec<TradePosition>>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        risk_reward: f64,
        dry_run: bool,
        non_trading_period_secs: i64,
        positino_size_ratio: f64,
        order_effective_duration_secs: i64,
        use_market_order: bool,
    ) -> Vec<FundManager> {
        let fund_manager_configurations = fund_config::get(&config.dex_name);
        let mut token_name_indices = HashMap::new();

        fund_manager_configurations
            .into_iter()
            .map(|(token_name, strategy, initial_amount)| {
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
                    &config.dex_name,
                    index,
                    &token_name,
                    open_positions_map.get(&fund_name).cloned(),
                    market_data,
                    strategy,
                    initial_amount * positino_size_ratio,
                    initial_amount,
                    risk_reward,
                    db_handler.clone(),
                    dex_client.clone(),
                    dry_run,
                    save_prices,
                    non_trading_period_secs,
                    order_effective_duration_secs,
                    use_market_order,
                )
            })
            .collect()
    }

    async fn create_dex_clinet(
        dex_router_api_key: &str,
        dex_router_url: &str,
    ) -> Option<DexClient> {
        let dex_client =
            DexClient::new(dex_router_api_key.to_owned(), dex_router_url.to_owned()).await;
        match dex_client {
            Ok(client) => Some(client),
            Err(e) => {
                log::error!("Failed to create a dex_client: {:?}", e);
                None
            }
        }
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
            10,    // grid_dize
            0.001, // gird step 0.1%
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
        // Check newly filled orders
        for (_, fund_manager) in self.state.fund_manager_map.iter_mut() {
            let filled_orders = self
                .state
                .dex_client
                .get_filled_orders(&self.config.dex_name, fund_manager.token_name())
                .await?;

            for order in filled_orders.orders {
                if fund_manager
                    .position_filled(
                        order.order_id.clone(),
                        order.filled_value.clone(),
                        order.filled_size.clone(),
                        order.filled_fee.clone(),
                    )
                    .await
                    .map_err(|_| Box::new(io::Error::new(ErrorKind::Other, "An error occurred")))?
                {
                    break;
                }
            }
        }

        // Get token prices
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
                        async move {
                            match get_price.await {
                                Some(price) => Ok((token_name, Some(price))),
                                None => Ok((token_name, None)),
                            }
                        }
                        .boxed(),
                    )
                }
            })
            .collect();

        let price_results = join_all(price_futures).await;

        let mut prices: HashMap<String, Option<f64>> = HashMap::new();
        for result in price_results {
            match result {
                Ok((token_name, price)) => {
                    prices.insert(token_name.to_owned(), price);
                }
                Err(err) => return Err(err),
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
            match result {
                Ok(()) => (),
                Err(err) => return Err(err),
            }
        }

        Ok(())
    }

    pub async fn reset_dex_client(&mut self) -> bool {
        log::info!("reset dex_client");
        let dex_client =
            Self::create_dex_clinet(&self.config.dex_router_api_key, &self.config.dex_router_url)
                .await;
        if dex_client.is_none() {
            return false;
        }

        let dex_client = dex_client.unwrap();

        for fund_manager in self.state.fund_manager_map.iter_mut() {
            fund_manager.1.reset_dex_client(dex_client.clone());
        }

        self.state.dex_client = dex_client;

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
        if let Ok(res) = self
            .state
            .dex_client
            .get_balance(&self.config.dex_name)
            .await
        {
            if let Some(equity) = res.equity {
                if let Ok(equity) = equity.parse::<f64>() {
                    return Ok(equity);
                }
            }
        }
        log::error!("failed to get the balance");
        return Err(());
    }
}
