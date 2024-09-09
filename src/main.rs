// main.rs

use chrono::{DateTime, FixedOffset, Utc};
use config::EnvConfig;
use debot_db::PricePoint;
use debot_utils::DateTimeUtils;
use env_logger::Builder;
use error_manager::ErrorManager;
use log::LevelFilter;
use rust_decimal::Decimal;
use std::env;
use std::io::Write;
use std::str::FromStr;
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
    let offset_seconds = env::var("TIMEZONE_OFFSET")
        .unwrap_or_else(|_| "3600".to_string())
        .parse::<i32>()
        .expect("Invalid TIMEZONE_OFFSET");

    let offset = FixedOffset::east_opt(offset_seconds).expect("Invalid offset");

    Builder::from_default_env()
        .format(move |buf, record| {
            let utc_now: DateTime<Utc> = Utc::now();
            let local_now = utc_now.with_timezone(&offset);
            writeln!(
                buf,
                "{} [{}] - {}",
                local_now.format("%Y-%m-%dT%H:%M:%S%z"),
                record.level(),
                record.args()
            )
        })
        .filter(
            None,
            LevelFilter::from_str(&env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string()))
                .unwrap_or(LevelFilter::Debug),
        )
        .init();

    // Load the configs
    let config = config::get_config_from_env().expect("Invalid configuration");

    // Set up the DB handler
    let max_position_counter = config.log_limit;
    let max_price_size = config.max_price_size * trade::TOKEN_LIST_SIZE;
    let db_handler = Arc::new(Mutex::new(
        DBHandler::new(
            max_position_counter,
            max_price_size,
            365,
            &config.mongodb_uri,
            &config.db_w_name,
            &config.db_r_name,
            config.back_test,
        )
        .await,
    ));

    // Read the last App state, and the market data from thd DB
    let (last_execution_time, last_equity, curcuit_break) =
        db_handler.lock().await.get_app_state().await;
    if curcuit_break {
        log::warn!("curcuit break!");
        loop {}
    }

    let price_size_to_load = if config.back_test {
        None
    } else {
        Some(max_price_size)
    };

    let price_market_data = db_handler
        .lock()
        .await
        .get_price_market_data(price_size_to_load, None)
        .await;

    // Initialize a trader instance
    let mut trader_instance = prepare_trader_instance(&config, db_handler, price_market_data).await;

    // Start main loop
    main_loop(&mut trader_instance, last_execution_time, last_equity, None).await
}

async fn prepare_trader_instance(
    config: &EnvConfig,
    db_handler: Arc<Mutex<DBHandler>>,
    price_market_data: HashMap<String, HashMap<String, Vec<PricePoint>>>,
) -> (DerivativeTrader, &EnvConfig, ErrorManager) {
    // todo: support multiple traders
    let (trading_interval, interval, dex_name) = &trader_config::get(config.strategy.as_ref())[0];

    // Create an error manager
    let error_manager = ErrorManager::new();

    let trader = DerivativeTrader::new(
        &dex_name,
        config.dry_run,
        *trading_interval,
        interval.clone(),
        config.interval_msec,
        config.max_price_size,
        db_handler,
        price_market_data.clone(),
        config.load_prices,
        config.save_prices,
        config.max_dd_ratio,
        config.close_order_effective_duration_secs,
        config.use_market_order,
        &config.rest_endpoint,
        &config.web_socket_endpoint,
        config.leverage,
        config.strategy.as_ref(),
        config.only_read_price,
        config.back_test,
    )
    .await;

    (trader, config, error_manager)
}

async fn main_loop(
    trader_instance: &mut (DerivativeTrader, &EnvConfig, ErrorManager),
    mut last_execution_time: Option<SystemTime>,
    mut last_equity: Option<Decimal>,
    mut last_dd_check_time: Option<SystemTime>,
) -> std::io::Result<()> {
    log::info!("main_loop() starts");

    let mut sigterm_stream =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    trader_instance.0.liquidate(false, "start").await;

    loop {
        let now = SystemTime::now();
        let one_day = Duration::from_secs(24 * 60 * 60);
        let loop_start = Instant::now();

        let (trader, config, error_manager) = trader_instance;

        // Check if last_execution_time is None or it's been more than one day
        if !config.back_test
            && last_execution_time.map_or(true, |last_time| {
                now.duration_since(last_time)
                    .map_or(false, |duration| duration > one_day)
            })
        {
            // Update the last_execution_time to now
            last_execution_time = Some(now);

            // Get and log yesterday's PNL;
            match trader.get_balance().await {
                Ok(balance) => {
                    let pnl = match last_equity {
                        Some(prev_balance) => balance - prev_balance,
                        None => Decimal::new(0, 0),
                    };
                    trader.db_handler().lock().await.log_pnl(pnl).await;
                    last_equity = Some(balance);
                }
                Err(_) => log::error!("Failed to get PNL"),
            }

            // Log the new last_execution_time and equity
            trader
                .db_handler()
                .lock()
                .await
                .log_app_state(last_execution_time, last_equity, false, None)
                .await;
        }

        // check DD
        let now = SystemTime::now();
        if !config.back_test
            && last_dd_check_time.map_or(true, |last_time| {
                now.duration_since(last_time)
                    .map_or(false, |duration| duration.as_secs() >= 3600) // 1 hour
            })
        {
            last_dd_check_time = Some(now);

            match trader.is_max_dd_occurred().await {
                Ok(is_dd) => {
                    if is_dd {
                        log::error!("Draw down!");
                        trader.liquidate(true, "Draw down").await;
                        trader
                            .db_handler()
                            .lock()
                            .await
                            .log_app_state(None, None, true, None)
                            .await;
                        log::info!("returned due to Draw down!");
                        return Ok(());
                    }
                }
                Err(_) => {
                    error_manager.save_first_error_time();
                    let _ = trader.reset_dex_client().await;
                }
            }
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
                trader.liquidate(true, "reboot").await;
            }
            std::process::exit(0);
        }

        let elapsed = loop_start.elapsed();
        let sleep_duration = if config.back_test {
            Duration::from_secs(0)
        } else {
            if let Some(remaining) =
                Duration::from_millis(config.interval_msec).checked_sub(elapsed)
            {
                remaining
            } else {
                Duration::from_secs(0)
            }
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
                trader.liquidate(true, "reboot").await;
            }
            std::process::exit(0);
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
        trader.liquidate(true, "Continous error").await;
        trader
            .db_handler()
            .lock()
            .await
            .log_app_state(
                None,
                None,
                false,
                Some(DateTimeUtils::get_current_datetime_string()),
            )
            .await;
        //std::process::exit(1);
        panic!("Intentionally crashing the app to trigger a restart.");
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
    use crate::config::get_hyperliquid_config_from_env;
    use dex_connector::{DexConnector, HyperliquidConnector, OrderSide};
    use rust_decimal::Decimal;
    use std::{env, sync::Arc, time::Duration};
    use tokio::time::sleep;

    #[ctor::ctor]
    fn setup() {
        env_logger::init();
    }

    async fn init_connector(dex_name: &str) -> Arc<dyn DexConnector> {
        let rest_endpoint = env::var("REST_ENDPOINT").expect("REST_ENDPOINT must be set");
        let web_socket_endpoint =
            env::var("WEB_SOCKET_ENDPOINT").expect("WEB_SOCKET_ENDPOINT must be set");

        let connector: Arc<dyn DexConnector> = match dex_name {
            "hyperliquid" => {
                let hyperliquid_config = get_hyperliquid_config_from_env().await.unwrap();
                Arc::new(
                    HyperliquidConnector::new(
                        &rest_endpoint,
                        &web_socket_endpoint,
                        &hyperliquid_config.agent_private_key,
                        &hyperliquid_config.evm_wallet_address,
                        hyperliquid_config.vault_address,
                    )
                    .await
                    .expect("Failed to initialize DexConnector"),
                )
            }
            _ => panic!("Not supported"),
        };
        connector.start().await.unwrap();
        connector
    }

    lazy_static! {
        static ref DEX_TEST_CONFIG: Vec<(&'static str, &'static str)> =
            vec![("hyperliquid", "BTC-USD")];
    }

    // #[tokio::test]
    // async fn get_last_trades() {
    //     for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let response = client.get_last_trades(symbol).await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());

    //         let response = client.clear_last_trades(symbol).await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());
    //     }
    // }

    #[tokio::test]
    async fn test_set_leverage() {
        for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
            let client = init_connector(dex_name).await;
            let response = client.set_leverage(symbol, 11).await;
            log::info!("{:?}", response);
            assert!(response.is_ok());
        }
    }

    // #[tokio::test]
    // async fn test_get_balance() {
    //     for (dex_name, _symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let response = client.get_balance().await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());
    //     }
    // }

    // #[tokio::test]
    // async fn test_get_ticker() {
    //     for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let response = client.get_ticker(symbol).await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());
    //     }
    // }

    // #[tokio::test]
    // async fn test_create_limit_order_buy() {
    //     for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let price = Decimal::new(30000, 0);
    //         let size = Decimal::new(5, 4);
    //         let response = client
    //             .create_order(symbol, size, OrderSide::Long, Some(price), None)
    //             .await
    //             .unwrap();

    //         let order_id = response.order_id;
    //         log::info!("order_id = {}", order_id);

    //         sleep(Duration::from_secs(3)).await;

    //         let response = client.cancel_order(symbol, &order_id).await;
    //         assert!(response.is_ok());
    //     }
    // }

    // #[tokio::test]
    // async fn test_create_limit_order_sell() {
    //     for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let price = Decimal::new(900000, 0);
    //         let size = Decimal::new(5, 4);
    //         let response = client
    //             .create_order(symbol, size, OrderSide::Short, Some(price), None)
    //             .await
    //             .unwrap();

    //         let order_id = response.order_id;
    //         log::info!("order_id = {}", order_id);

    //         sleep(Duration::from_secs(3)).await;

    //         let response = client.cancel_all_orders(Some(symbol.to_string())).await;
    //         assert!(response.is_ok());
    //     }
    // }

    // #[tokio::test]
    // async fn test_create_market_order_buy() {
    //     for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let size = Decimal::new(5, 4);
    //         let response = client
    //             .create_order(symbol, size, OrderSide::Long, None)
    //             .await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());

    //         sleep(Duration::from_secs(3)).await;

    //         let response = client.get_filled_orders(symbol).await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());

    //         client
    //             .close_all_positions(Some(symbol.to_string()))
    //             .await
    //             .unwrap();
    //     }
    // }

    // #[tokio::test]
    // async fn test_create_market_order_sell() {
    //     for (dex_name, symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         let size = Decimal::new(5, 4);
    //         let response = client
    //             .create_order(symbol, size, OrderSide::Short, None)
    //             .await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());

    //         sleep(Duration::from_secs(3)).await;

    //         let response = client.get_filled_orders(symbol).await;
    //         log::info!("{:?}", response);
    //         assert!(response.is_ok());

    //         client.close_all_positions(None).await.unwrap();
    //     }
    // }

    // #[tokio::test]
    // async fn test_close_all_positions() {
    //     for (dex_name, _symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         client.close_all_positions(None).await.unwrap();
    //     }
    // }

    // #[tokio::test]
    // async fn test_cancel_all_orders() {
    //     for (dex_name, _symbol) in DEX_TEST_CONFIG.iter() {
    //         let client = init_connector(dex_name).await;
    //         client.cancel_all_orders(None).await.unwrap();
    //     }
    // }
}
