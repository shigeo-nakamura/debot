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

use super::fund_config;
use super::DBHandler;
use super::FundManager;
use super::TransactionLog;

#[derive(Clone)]
pub struct TradingPeriod {
    short_term_hour: usize,
    medium_term_hour: usize,
    long_term_hour: usize,
}

impl TradingPeriod {
    pub fn new(short_term_hour: usize, medium_term_hour: usize, long_term_hour: usize) -> Self {
        Self {
            short_term_hour,
            medium_term_hour,
            long_term_hour,
        }
    }
}

#[derive(Clone)]
struct DerivativeTraderConfig {
    name: String,
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
    max_price_size: u32,
    interval: u64,
}

struct DerivativeTraderState {
    db_handler: Arc<Mutex<DBHandler>>,
    fund_manager_map: HashMap<String, FundManager>,
}

pub struct DerivativeTrader {
    state: DerivativeTraderState,
}

impl DerivativeTrader {
    pub async fn new(
        name: &str,
        dry_run: bool,
        trading_period: TradingPeriod,
        max_price_size: u32,
        interval: u64,
        risk_reward: f64,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        open_positions_map: HashMap<String, HashMap<String, TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        encrypted_api_key: &str,
        dex_router_url: &str,
    ) -> Self {
        const SECONDS_IN_HOUR: usize = 3600;
        let config = DerivativeTraderConfig {
            name: name.to_owned(),
            short_trade_period: trading_period.short_term_hour * SECONDS_IN_HOUR
                / interval as usize,
            medium_trade_period: trading_period.medium_term_hour * SECONDS_IN_HOUR
                / interval as usize,
            long_trade_period: trading_period.long_term_hour * SECONDS_IN_HOUR / interval as usize,
            max_price_size,
            interval,
        };

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
        )
        .await;

        Self { state }
    }

    async fn create_fund_managers(
        config: &DerivativeTraderConfig,
        db_client: &Arc<Mutex<ClientHolder>>,
        transaction_log: &Arc<TransactionLog>,
        encrypted_api_key: &str,
        dex_router_url: &str,
        open_positions_map: &HashMap<String, HashMap<String, TradePosition>>,
        price_market_data: &HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        risk_reward: f64,
        dry_run: bool,
    ) -> Vec<FundManager> {
        let fund_manager_configurations = fund_config::get();
        let db_handler = Arc::new(Mutex::new(DBHandler::new(
            db_client.clone(),
            transaction_log.clone(),
        )));
        let dex_client =
            DexClient::new(encrypted_api_key.to_owned(), dex_router_url.to_owned()).await;
        if let Err(e) = dex_client {
            log::error!("{:?}", e);
            return vec![];
        }
        let dex_client = dex_client.unwrap();

        fund_manager_configurations
            .into_iter()
            .filter_map(|(token_name, strategy, initial_amount, trading_amount)| {
                let fund_name = format!("{:?}-{}-{}", strategy, token_name, initial_amount);

                let mut market_data = Self::create_market_data(config.clone());

                if load_prices {
                    Self::restore_market_data(
                        &mut market_data,
                        &config.name,
                        &token_name,
                        &price_market_data,
                    );
                }

                Some(FundManager::new(
                    &fund_name,
                    &token_name,
                    open_positions_map.get(&fund_name).cloned(),
                    market_data,
                    strategy,
                    config.short_trade_period,
                    trading_amount,
                    initial_amount,
                    risk_reward,
                    db_handler.clone(),
                    dex_client.clone(),
                    dry_run,
                    save_prices,
                ))
            })
            .collect()
    }

    async fn initialize_state(
        config: DerivativeTraderConfig,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        encrypted_api_key: &str,
        dex_router_url: &str,
        open_positions_map: HashMap<String, HashMap<String, TradePosition>>,
        price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
        load_prices: bool,
        save_prices: bool,
        risk_reward: f64,
        dry_run: bool,
    ) -> DerivativeTraderState {
        let fund_managers = Self::create_fund_managers(
            &config,
            &db_client,
            &transaction_log,
            encrypted_api_key,
            dex_router_url,
            &open_positions_map,
            &price_market_data,
            load_prices,
            save_prices,
            risk_reward,
            dry_run,
        )
        .await;

        let mut state = DerivativeTraderState {
            db_handler: Arc::new(Mutex::new(DBHandler::new(
                db_client.clone(),
                transaction_log.clone(),
            ))),
            fund_manager_map: HashMap::new(),
        };

        for fund_manager in fund_managers {
            state
                .fund_manager_map
                .insert(fund_manager.name().to_owned(), fund_manager);
        }

        state
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

    pub fn is_any_fund_liquidated(&self) -> bool {
        for fund_manager in self.state.fund_manager_map.values() {
            if fund_manager.is_liquidated() {
                return true;
            }
        }
        false
    }

    pub async fn liquidate(&mut self) {
        for fund_manager in self.state.fund_manager_map.values_mut() {
            fund_manager.begin_liquidate();
        }

        self.state
            .db_handler
            .lock()
            .await
            .log_liquidate_time()
            .await;
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
                    market_data.add_price(price_point.price, Some(price_point.timestamp));
                }
            }
        }
    }

    fn create_market_data(config: DerivativeTraderConfig) -> MarketData {
        MarketData::new(
            config.name.to_owned(),
            config.short_trade_period,
            config.medium_trade_period,
            config.long_trade_period,
            config.max_price_size as usize,
            config.interval,
        )
    }

    pub fn db_handler(&self) -> &Arc<Mutex<DBHandler>> {
        &self.state.db_handler
    }
}
