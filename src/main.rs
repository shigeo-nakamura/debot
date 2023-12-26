// main.rs

use config::EnvConfig;
use debot_market_analyzer::PricePoint;
use error_manager::ErrorManager;
use tokio::sync::Mutex;
use tokio::time::Instant;
use trade::{trader_config, DerivativeTrader};

use crate::trade::DBHandler;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

mod config;
mod error_manager;
mod trade;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Init logging
    env_logger::init();

    // Load the configs
    let config = config::get_config_from_env()
        .await
        .expect("Invalid configuration");

    // Set up the DB handler
    let db_handler = Arc::new(Mutex::new(
        DBHandler::new(
            config.log_limit,
            config.max_price_size * trade::TOKEN_LIST_SIZE as u32,
            config.log_limit,
            &config.mongodb_uri,
            &config.db_name,
        )
        .await,
    ));

    // Read the last App state, and the market data from thd DB
    let (last_execution_time, last_equity, curcuit_break) =
        db_handler.lock().await.get_app_state().await;
    if curcuit_break {
        loop {}
    }

    let price_market_data = db_handler.lock().await.get_price_market_data().await;

    // Initialize a trader instance
    let mut trader_instance = prepare_trader_instance(&config, db_handler, price_market_data).await;

    // Start main loop
    main_loop(&mut trader_instance, last_execution_time, last_equity).await
}

async fn prepare_trader_instance(
    config: &EnvConfig,
    db_handler: Arc<Mutex<DBHandler>>,
    price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
) -> (DerivativeTrader, &EnvConfig, ErrorManager) {
    let (prediction_interval, interval, dex_name) = trader_config::get();

    // Read open positions from the DB
    let open_positions_map = db_handler.lock().await.get_open_positions_map().await;

    // Create an error manager
    let error_manager = ErrorManager::new();

    let trader = DerivativeTrader::new(
        &dex_name,
        config.dry_run,
        prediction_interval,
        interval,
        config.max_price_size,
        config.risk_reward,
        db_handler,
        open_positions_map,
        price_market_data.clone(),
        config.load_prices,
        config.save_prices,
        &config.dex_router_api_key,
        &config.dex_router_url,
        config.non_trading_period_secs,
        config.position_size_ratio,
    )
    .await;

    (trader, config, error_manager)
}

async fn main_loop(
    trader_instance: &mut (DerivativeTrader, &EnvConfig, ErrorManager),
    mut last_execution_time: Option<SystemTime>,
    mut last_equity: Option<f64>,
) -> std::io::Result<()> {
    log::info!("main_loop() starts");

    let mut sigterm_stream =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    loop {
        let now = SystemTime::now();
        let one_day = Duration::from_secs(24 * 60 * 60);
        let loop_start = Instant::now();

        let (trader, config, error_manager) = trader_instance;

        // Check if last_execution_time is None or it's been more than one day
        if last_execution_time.map_or(true, |last_time| {
            now.duration_since(last_time)
                .map_or(false, |duration| duration > one_day)
        }) {
            // Update the last_execution_time to now
            last_execution_time = Some(now);

            // Get and log yesterday's PNL
            if let Ok(res) = trader.dex_client().get_balance(trader.dex_name()).await {
                if let Some(balance) = res.balance {
                    if let Ok(balance) = balance.parse::<f64>() {
                        let pnl = match last_equity {
                            Some(prev_balance) => balance - prev_balance,
                            None => 0.0,
                        };
                        trader.db_handler().lock().await.log_pnl(pnl).await;
                        last_equity = Some(balance);
                    } else {
                        log::error!("Failed to get PNL");
                    }
                }
            } else {
                log::error!("Failed to get PNL");
            }

            // Log the new last_execution_time and equity
            trader
                .db_handler()
                .lock()
                .await
                .log_app_state(last_execution_time, last_equity, false)
                .await;
        }

        // Create a non-mutable borrow for the function
        let trader_future = Box::pin(handle_trader_activities(trader, config, error_manager));

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
            _ = trader_future => {
                // The trader task has completed.
                // Handle the result or re-schedule as needed.
                exit = false;
            }
        }

        if exit {
            if config.liquidate_when_exit {
                trader.liquidate("reboot").await;
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
            if config.liquidate_when_exit {
                trader.liquidate("reboot").await;
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
        trader.liquidate("Continous Errors").await;
        trader
            .db_handler()
            .lock()
            .await
            .log_app_state(None, None, false)
            .await;
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

    lazy_static! {
        static ref DEX_TEST_CONFIG: Vec<(&'static str, &'static str)> =
            vec![("apex", "BTC-USDC"), ("mufex", "BTC-USDT")];
    }

    #[tokio::test]
    async fn test_get_ticket() {
        let client = init_client().await;
        for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
            let response = client.get_ticker(dex_name, symbol).await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_get_balance() {
        for (dex_name, _symbol) in DEX_TEST_CONFIG.iter() {
            let client = init_client().await;
            let response = client.get_balance(dex_name).await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_create_order_buy() {
        for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
            let client = init_client().await;
            let response = client.get_ticker(dex_name, symbol).await;
            let price = response.unwrap().price.unwrap().parse::<f64>().unwrap();
            let response = client
                .create_order(dex_name, symbol, "0.001", "BUY", Some(price.to_string()))
                .await;
            log::info!("{:?}", response);
            assert!(response.is_ok());

            let response = client.get_filled_orders(&dex_name, symbol).await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_create_order_sell() {
        for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
            let client = init_client().await;
            let response = client.get_ticker(dex_name, symbol).await;
            let price = response.unwrap().price.unwrap().parse::<f64>().unwrap();
            let response = client
                .create_order(dex_name, symbol, "0.001", "SELL", Some(price.to_string()))
                .await;
            log::info!("{:?}", response);
            assert!(response.is_ok());

            let response = client.get_filled_orders(&dex_name, symbol).await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_close_all_positions() {
        for (dex_name, _symbol) in DEX_TEST_CONFIG.iter() {
            if *dex_name == "mufex" {
                continue; // Not supported
            }
            let client = init_client().await;
            let response = client.close_all_positions(dex_name, None).await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }

    #[tokio::test]
    async fn test_close_all_positions_for_specific_token() {
        for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
            let client = init_client().await;
            let response = client
                .close_all_positions(dex_name, Some(symbol.to_string()))
                .await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }
}
