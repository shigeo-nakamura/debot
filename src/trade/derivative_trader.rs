// derivative_trader.rs

use debot_market_analyzer::MarketData;
use debot_market_analyzer::PricePoint;
use debot_position_manager::TradePosition;
use dex_client::DexClient;
use futures::future::join_all;
use shared_mongodb::ClientHolder;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::trade::fund_config::TOKEN_LIST_SIZE;

use super::fund_config;
use super::DBHandler;
use super::FundManager;
use super::TransactionLog;

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
    name: String,
    short_trade_period: usize,
    long_trade_period: usize,
    max_price_size: u32,
    encrypted_api_key: String,
    dex_router_url: String,
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
        name: &str,
        dry_run: bool,
        prediction_interval: usize,
        interval: SampleInterval,
        max_price_size: u32,
        risk_reward: f64,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        open_positions_map: HashMap<String, Vec<TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        encrypted_api_key: &str,
        dex_router_url: &str,
    ) -> Self {
        const SECONDS_IN_MINUTE: usize = 60;
        let config = DerivativeTraderConfig {
            name: name.to_owned(),
            short_trade_period: interval.short_term * SECONDS_IN_MINUTE,
            long_trade_period: interval.long_term * SECONDS_IN_MINUTE,
            max_price_size: max_price_size * TOKEN_LIST_SIZE as u32,
            encrypted_api_key: encrypted_api_key.to_owned(),
            dex_router_url: dex_router_url.to_owned(),
        };
        let interval = prediction_interval * SECONDS_IN_MINUTE;

        let state = Self::initialize_state(
            config.clone(),
            db_client,
            transaction_log,
            encrypted_api_key,
            dex_router_url,
            open_positions_map,
            price_market_data,
            load_prices,
            save_prices,
            risk_reward,
            dry_run,
            interval,
        )
        .await;

        Self { config, state }
    }

    async fn initialize_state(
        config: DerivativeTraderConfig,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        encrypted_api_key: &str,
        dex_router_url: &str,
        open_positions_map: HashMap<String, Vec<TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        risk_reward: f64,
        dry_run: bool,
        prediction_interval: usize,
    ) -> DerivativeTraderState {
        let dex_client = Self::create_dex_clinet(encrypted_api_key, dex_router_url)
            .await
            .expect("Failed to initialize DexClient");

        let fund_managers = Self::create_fund_managers(
            &config,
            &db_client,
            &dex_client,
            &transaction_log,
            &open_positions_map,
            &price_market_data,
            load_prices,
            save_prices,
            risk_reward,
            dry_run,
            prediction_interval,
        )
        .await;

        let mut state = DerivativeTraderState {
            db_handler: Arc::new(Mutex::new(DBHandler::new(
                db_client.clone(),
                transaction_log.clone(),
            ))),
            dex_client,
            fund_manager_map: HashMap::new(),
        };

        for fund_manager in fund_managers {
            state
                .fund_manager_map
                .insert(fund_manager.name().to_owned(), fund_manager);
        }

        state
    }

    async fn create_fund_managers(
        config: &DerivativeTraderConfig,
        db_client: &Arc<Mutex<ClientHolder>>,
        dex_client: &DexClient,
        transaction_log: &Arc<TransactionLog>,
        open_positions_map: &HashMap<String, Vec<TradePosition>>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        risk_reward: f64,
        dry_run: bool,
        prediction_interval: usize,
    ) -> Vec<FundManager> {
        let fund_manager_configurations = fund_config::get();
        let db_handler = Arc::new(Mutex::new(DBHandler::new(
            db_client.clone(),
            transaction_log.clone(),
        )));

        let mut token_name_indices = HashMap::new();

        fund_manager_configurations
            .into_iter()
            .map(|(token_name, strategy, initial_amount, trading_amount)| {
                let fund_name = format!(
                    "{:?}-{}-{}-{}",
                    strategy, token_name, config.short_trade_period, config.long_trade_period
                );

                let mut market_data = Self::create_market_data(config.clone());

                if load_prices {
                    Self::restore_market_data(
                        &mut market_data,
                        &config.name,
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
                    strategy,
                    prediction_interval,
                    trading_amount,
                    initial_amount,
                    risk_reward,
                    db_handler.clone(),
                    dex_client.clone(),
                    dry_run,
                    save_prices,
                )
            })
            .collect()
    }

    async fn create_dex_clinet(encrypted_api_key: &str, dex_router_url: &str) -> Option<DexClient> {
        let dex_client =
            DexClient::new(encrypted_api_key.to_owned(), dex_router_url.to_owned()).await;
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
            config.name.to_owned(),
            config.short_trade_period,
            config.long_trade_period,
            config.max_price_size as usize,
        )
    }

    pub async fn find_chances(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let futures: Vec<_> = self
            .state
            .fund_manager_map
            .values_mut()
            .map(|fund_manager| fund_manager.find_chances())
            .collect();

        let results: Vec<Result<_, _>> = join_all(futures).await;

        for result in results {
            match result {
                Ok(()) => (),
                Err(err) => return Err(err),
            }
        }

        Ok(())
    }

    pub async fn reset_dex_client(&mut self) -> bool {
        let dex_client =
            Self::create_dex_clinet(&self.config.encrypted_api_key, &self.config.dex_router_url)
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

    pub fn db_handler(&self) -> &Arc<Mutex<DBHandler>> {
        &self.state.db_handler
    }

    pub fn dex_client(&self) -> &DexClient {
        &self.state.dex_client
    }
}
