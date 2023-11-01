// main.rs

use config::EnvConfig;
use db::create_unique_index;
use debot_market_analyzer::PricePoint;
use debot_position_manager::TradePosition;
use error_manager::ErrorManager;
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use shared_mongodb::ClientHolder;
use tokio::sync::Mutex;
use trade::derivative_trader::TradingPeriod;
use trade::{trader_config, DerivativeTrader, TransactionLog};

use crate::trade::DBHandler;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

mod config;
mod db;
mod error_manager;
mod trade;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Load the configs
    let config = config::get_config_from_env().expect("Invalid configuration");

    // Set up the DB client holder
    let mut client_options = match ClientOptions::parse(&config.mongodb_uri).await {
        Ok(client_options) => client_options,
        Err(e) => {
            panic!("{:?}", e);
        }
    };
    let tls_options = TlsOptions::builder().build();
    client_options.tls = Some(Tls::Enabled(tls_options));
    let client_holder = Arc::new(Mutex::new(ClientHolder::new(client_options)));

    let db_name = &config.db_name;
    let db = shared_mongodb::database::get(&client_holder, &db_name)
        .await
        .unwrap();
    create_unique_index(&db)
        .await
        .expect("Error creating unique index");

    // Set up the transaction log
    let last_position_counter =
        TransactionLog::get_last_transaction_id(&db, db::CounterType::Position).await;
    let last_price_counter =
        TransactionLog::get_last_transaction_id(&db, db::CounterType::Price).await;
    let last_balance_counter =
        TransactionLog::get_last_transaction_id(&db, db::CounterType::Pnl).await;

    let transaction_log = Arc::new(TransactionLog::new(
        config.log_limit,
        config.max_price_size,
        config.log_limit,
        last_position_counter,
        last_price_counter,
        last_balance_counter,
        &db_name,
    ));

    // Read the last App state
    let app_state = TransactionLog::get_app_state(&db).await;

    // Read the price market_data
    let price_market_data = TransactionLog::get_price_market_data(&db).await;

    // Initialize a vector to hold trader instances
    let mut trader_instances = prepare_trader_instances(
        &config,
        client_holder.clone(),
        transaction_log.clone(),
        price_market_data,
    )
    .await;

    main_loop(
        &mut trader_instances,
        &config,
        app_state.last_execution_time,
    )
    .await
}

async fn prepare_trader_instances(
    config: &EnvConfig,
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
    price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
) -> Vec<(DerivativeTrader, &EnvConfig, ErrorManager)> {
    // Read open positions from the DB
    let open_positions_map =
        DBHandler::get_open_positions_map(transaction_log.clone(), client_holder.clone()).await;

    let mut trader_instances = Vec::new();

    for (trading_period, trader_name) in trader_config::get() {
        let trader_instance = prepare_algorithm_trader_instance(
            trader_name.to_string(),
            config,
            client_holder.clone(),
            transaction_log.clone(),
            trading_period.clone(),
            price_market_data.clone(),
            open_positions_map.clone(),
        )
        .await;
        trader_instances.push(trader_instance);
    }

    trader_instances
}

async fn prepare_algorithm_trader_instance(
    trader_name: String,
    config: &EnvConfig,
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
    tradeding_preiod: TradingPeriod,
    price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
    open_positions_map: HashMap<String, HashMap<String, TradePosition>>,
) -> (DerivativeTrader, &EnvConfig, ErrorManager) {
    // Create an error manager
    let error_manager = ErrorManager::new();

    let trader = DerivativeTrader::new(
        &trader_name,
        config.dry_run,
        tradeding_preiod,
        config.max_price_size,
        config.interval,
        config.risk_reward,
        client_holder.clone(),
        transaction_log,
        open_positions_map,
        price_market_data.clone(),
        config.load_prices,
        config.save_prices,
        &config.encrypted_api_key,
        &config.dex_router_url,
    )
    .await;

    (trader, config, error_manager)
}

async fn main_loop(
    trader_instances: &mut Vec<(DerivativeTrader, &EnvConfig, ErrorManager)>,
    config: &EnvConfig,
    mut last_execution_time: Option<SystemTime>,
) -> std::io::Result<()> {
    let one_day = Duration::from_secs(24 * 60 * 60);
    let interval = config.interval as f64 / trader_instances.len() as f64;

    let execution_time = last_execution_time.unwrap_or(SystemTime::UNIX_EPOCH);
    last_execution_time = Some(execution_time);

    log::info!("main_loop() starts");

    loop {
        let now = SystemTime::now();

        for (trader, config, error_manager) in trader_instances.iter_mut() {
            if trader.is_any_fund_liquidated() {
                log::info!("Paused");
                loop {}
            }

            if now.duration_since(last_execution_time.unwrap()).unwrap() > one_day {
                // log last_execution_time
                last_execution_time = Some(now);
                trader
                    .db_handler()
                    .lock()
                    .await
                    .log_app_state(last_execution_time, false)
                    .await;

                // get and log yesterday's PNL
                if let Ok(res) = trader.dex_client().get_yesterday_pnl().await {
                    if let Ok(pnl) = res.data.parse::<f64>() {
                        trader.db_handler().lock().await.log_pnl(pnl).await;
                    } else {
                        log::error!("Failed to log PNL");
                    }
                } else {
                    log::error!("Failed to get PNL");
                }
            }

            if error_manager.get_error_count() >= config.max_error_count {
                log::error!("Error count reached the limit");
                trader.liquidate().await;
            }

            match trader.find_chances().await {
                Ok(_) => {
                    error_manager.reset_error_count();
                }
                Err(e) => {
                    log::error!("Error while finding opportunities: {}", e);
                    error_manager.increment_error_count();
                }
            };

            if let Err(_) = handle_sleep_and_signal(interval).await {
                return Ok(());
            }
        }
    }
}

async fn handle_sleep_and_signal(interval: f64) -> Result<(), &'static str> {
    let sleep_fut = tokio::time::sleep(Duration::from_secs_f64(interval));
    let mut sigterm_stream =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    let ctrl_c_fut = tokio::signal::ctrl_c();
    tokio::select! {
        _ = sleep_fut => {
            // continue to the next iteration of loop
        },
        _ = sigterm_stream.recv() => {
            log::info!("SIGTERM received. Shutting down...");
            return Err("SIGTERM received");
        },
        _ = ctrl_c_fut => {
            log::info!("SIGINT received. Shutting down...");
            return Err("SIGINT received");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use dex_client::DexClient;
    use std::env;

    #[ctor::ctor]
    fn setup() {
        env_logger::init();
    }

    async fn init_client() -> DexClient {
        let api_key = env::var("ENCRYPTED_API_KEY").expect("API_KEY must be set");
        let dex_router_url = env::var("DEX_ROUTER_URL").expect("DEX_ROUTER_URL must be set");
        DexClient::new(api_key, dex_router_url)
            .await
            .expect("Failed to initialize DexClient")
    }

    #[tokio::test]
    async fn test_get_ticker() {
        let client = init_client().await;
        let response = client.get_ticker("BTC-USDC").await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_get_yesterday_pnl() {
        let client = init_client().await;
        let response = client.get_yesterday_pnl().await;
        log::info!("{:?}", response);
        assert!(response.is_ok());
    }
}
