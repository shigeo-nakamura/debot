// main.rs

use config::EnvConfig;
use db::create_unique_index;
use debot_market_analyzer::PricePoint;
use debot_position_manager::TradePosition;
use error_manager::ErrorManager;
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use shared_mongodb::ClientHolder;
use tokio::sync::Mutex;
use tokio::time::Instant;
use trade::derivative_trader::SampleInterval;
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
    let config = config::get_config_from_env()
        .await
        .expect("Invalid configuration");

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
    let last_pnl_counter = TransactionLog::get_last_transaction_id(&db, db::CounterType::Pnl).await;

    let transaction_log = Arc::new(TransactionLog::new(
        config.log_limit,
        config.max_price_size,
        config.log_limit,
        last_position_counter,
        last_price_counter,
        last_pnl_counter,
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

    for (prediction_interval, interval, trader_name) in trader_config::get() {
        let trader_instance = prepare_algorithm_trader_instance(
            trader_name.to_string(),
            config,
            client_holder.clone(),
            transaction_log.clone(),
            prediction_interval,
            interval.clone(),
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
    prediction_interval: usize,
    interval: SampleInterval,
    price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
    open_positions_map: HashMap<String, Vec<TradePosition>>,
) -> (DerivativeTrader, &EnvConfig, ErrorManager) {
    // Create an error manager
    let error_manager = ErrorManager::new();

    let trader = DerivativeTrader::new(
        &trader_name,
        config.dry_run,
        prediction_interval,
        interval,
        config.max_price_size,
        config.risk_reward,
        client_holder.clone(),
        transaction_log,
        open_positions_map,
        price_market_data.clone(),
        config.load_prices,
        config.save_prices,
        &config.dex_router_api_key,
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
    log::info!("main_loop() starts");

    let mut sigterm_stream =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    loop {
        let now = SystemTime::now();
        let one_day = Duration::from_secs(24 * 60 * 60);
        let loop_start = Instant::now();

        // Check if last_execution_time is None or it's been more than one day
        if last_execution_time.map_or(true, |last_time| {
            now.duration_since(last_time)
                .map_or(false, |duration| duration > one_day)
        }) {
            // Update the last_execution_time to now
            last_execution_time = Some(now);

            // Log the new last_execution_time
            let trader = &trader_instances[0].0;
            trader
                .db_handler()
                .lock()
                .await
                .log_app_state(last_execution_time, false)
                .await;

            // Get and log yesterday's PNL
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

        let mut trader_futures = Vec::new();

        {
            for (trader, config, error_manager) in trader_instances.iter_mut() {
                // Create a non-mutable borrow for the function
                let trader_future =
                    Box::pin(handle_trader_activities(trader, config, error_manager));
                trader_futures.push(trader_future);
            }
        }

        let mut exit;
        tokio::select! {
            _ = sigterm_stream.recv() => {
                log::info!("SIGTERM received. Shutting down...");
                exit = true;
            },
            _ = tokio::signal::ctrl_c() => {
                log::info!("SIGINT received. Shutting down...");
                exit = true;
            },
            _ = futures::future::select_all(trader_futures) => {
                // One of the trader tasks has completed.
                // Handle the result or re-schedule as needed.
                exit = false;
            },
        }

        if exit {
            for (trader, _config, _error_manager) in trader_instances.iter_mut() {
                trader.liquidate().await;
            }
            return Ok(());
        }

        let elapsed = loop_start.elapsed();
        let sleep_duration = if let Some(remaining) =
            Duration::from_millis(config.interval_msec).checked_sub(elapsed)
        {
            remaining
        } else {
            Duration::from_secs(0)
        };

        let sleep = tokio::time::sleep(sleep_duration);
        tokio::pin!(sleep);

        tokio::select! {
            _ = sigterm_stream.recv() => {
                log::info!("SIGTERM received. Shutting down...");
                exit = true;
            },
            _ = tokio::signal::ctrl_c() => {
                log::info!("SIGINT received. Shutting down...");
                exit = true;
            },
            _ = &mut sleep => {
                exit = false;
            },
        }

        if exit {
            for (trader, _config, _error_manager) in trader_instances.iter_mut() {
                trader.liquidate().await;
            }
            return Ok(());
        }
    }
}

async fn handle_trader_activities(
    trader: &mut DerivativeTrader,
    config: &EnvConfig,
    error_manager: &mut ErrorManager,
) {
    let error_duration = Duration::from_secs(config.max_error_duration);

    // Check if the error duration has passed
    if error_manager.has_error_duration_passed(error_duration) {
        log::error!("Error duration exceeded the limit");
        trader.liquidate().await;
        loop {}
    }

    match trader.find_chances().await {
        Ok(_) => {
            error_manager.reset_error_time();
        }
        Err(e) => {
            log::error!("Error while finding opportunities: {}", e);
            error_manager.save_first_error_time();
            let _ = trader.reset_dex_client().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use debot_utils::decrypt_data_with_kms;
    use dex_client::DexClient;
    use std::env;

    #[ctor::ctor]
    fn setup() {
        env_logger::init();
    }

    async fn init_client() -> DexClient {
        let encrypted_data_key = env::var("ENCRYPTED_DATA_KEY")
            .expect("ENCRYPTED_DATA_KEY must be set")
            .replace(" ", ""); // Remove whitespace characters

        let encrypted_dex_router_api_key = env::var("ENCRYPTED_DEX_ROUTER_API_KEY")
            .expect("ENCRYPTED_DEX_ROUTER_API_KEY must be set")
            .replace(" ", ""); // Remove whitespace characters

        let dex_router_api_key =
            decrypt_data_with_kms(encrypted_data_key, encrypted_dex_router_api_key)
                .await
                .unwrap();
        let dex_router_api_key = String::from_utf8(dex_router_api_key).unwrap();

        let dex_router_url = env::var("DEX_ROUTER_URL").expect("DEX_ROUTER_URL must be set");

        DexClient::new(dex_router_api_key, dex_router_url)
            .await
            .expect("Failed to initialize DexClient")
    }

    #[tokio::test]
    async fn test_get_yesterday_pnl() {
        let client = init_client().await;
        let response = client.get_yesterday_pnl().await;
        log::info!("{:?}", response);
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_create_order_buy() {
        let client = init_client().await;
        let response = client.get_ticker("BTCUSDC").await;
        let price = response.unwrap().price.parse::<f64>().unwrap();
        let response = client
            .create_order("BTC-USDC", "0.001", "BUY", Some(price.to_string()))
            .await;
        log::info!("{:?}", response);
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_create_order_sell() {
        let client = init_client().await;
        let response = client.get_ticker("BTCUSDC").await;
        let price = response.unwrap().price.parse::<f64>().unwrap();
        let response = client
            .create_order("BTC-USDC", "0.001", "SELL", Some(price.to_string()))
            .await;
        log::info!("{:?}", response);
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_close_all_positions() {
        let client = init_client().await;
        let response = client.close_all_positions(None).await;
        log::info!("{:?}", response);
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_close_all_positions_for_specific_token() {
        let client = init_client().await;
        let response = client
            .close_all_positions(Some("SOL-USDC".to_string()))
            .await;
        log::info!("{:?}", response);
        assert!(response.is_ok());
    }
}
