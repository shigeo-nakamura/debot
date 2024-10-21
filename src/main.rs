// main.rs

use backtest::download_data;
use chrono::{DateTime, FixedOffset, Utc};
use config::EnvConfig;
use debot_db::{ModelParams, PricePoint, TransactionLog};
use debot_market_analyzer::{TradingStrategy, TrendType};
use debot_ml::{grid_search_and_train_classifier, grid_search_and_train_regressor};
use debot_utils::DateTimeUtils;
use env_logger::Builder;
use error_manager::ErrorManager;
use log::LevelFilter;
use rust_decimal::Decimal;
use std::env;
use std::io::Write;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tokio::time::Instant;
use trade::{trader_config, DerivativeTrader};

use crate::trade::DBHandler;
use csv::Writer;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

mod backtest;
mod config;
mod email_client;
mod error_manager;
mod trade;

static MAX_ELAPSED: AtomicU64 = AtomicU64::new(0);

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

    let args: Vec<String> = std::env::args().collect();

    if args.len() == 1 {
        log::info!("No command provided. Running default program...");
        return run_default_program().await;
    }

    if args.len() < 3 {
        eprintln!("Usage: <command> [key]");
        return Ok(());
    }

    let command = &args[1];
    let key = &args[2];
    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");

    log::info!("Received key: {}", key);

    match command.as_str() {
        "copy" => {
            let db_w_name = key;
            let db_r_name = env::var("DB_R_NAME").expect("DB_R_NAME must be set");
            let transaction_log = TransactionLog::new(
                Some(0),
                Some(0),
                Some(0),
                &mongodb_uri,
                &db_r_name,
                &db_w_name,
                false,
            )
            .await;
            let db_r = transaction_log.get_r_db().await.expect("db_r is none");
            let db_w = transaction_log.get_w_db().await.expect("db_w is none");
            TransactionLog::copy_price(&db_r, &db_w, None).await;
            log::info!("Price copied to {}", key);
        }
        "get" => {
            let db_w_name = "unused";
            let db_r_name = env::var("DB_R_NAME").expect("DB_R_NAME must be set");
            let transaction_log = TransactionLog::new(
                Some(0),
                Some(0),
                Some(0),
                &mongodb_uri,
                &db_r_name,
                &db_w_name,
                false,
            )
            .await;
            let db = transaction_log.get_r_db().await.expect("db is none");
            let positions = TransactionLog::get_all_open_positions(&db).await;

            let mut wtr = Writer::from_writer(File::create(&key)?);

            wtr.write_record(&["position_type", "pnl"])?;

            for position in positions {
                wtr.write_record(&[
                    position.position_type.clone(),
                    position.pnl.round_dp(3).to_string(),
                ])?;
            }

            wtr.flush()?;

            log::info!("Positions saved to {}", key);
        }
        "save" => {
            let db_w_name = "unused";
            let db_r_name = env::var("DB_R_NAME").expect("DB_R_NAME must be set");
            let transaction_log = TransactionLog::new(
                Some(0),
                Some(0),
                Some(0),
                &mongodb_uri,
                &db_r_name,
                &db_w_name,
                false,
            )
            .await;
            let db = transaction_log.get_r_db().await.expect("db is none");
            let prices = TransactionLog::get_price_market_data(&db, None, None, true).await;

            let file_path = &key;
            let file = File::create(file_path)?;
            serde_json::to_writer(file, &prices)?;

            log::info!("prices saved to {}", key);
        }
        "train" => {
            let db_w_name = env::var("DB_W_NAME").expect("DB_W_NAME must be set");
            let db_r_names = env::var("DB_R_NAMES").expect("DB_R_NAMES must be set");
            let db_r_names: Vec<&str> = db_r_names.split(',').collect();
            let path_to_models = env::var("PATH_TO_MODELS").ok();
            let (strategy, file_key) =
                match env::var("TRADING_STRATEGY").unwrap_or_default().as_str() {
                    "meanreversion" => (
                        TradingStrategy::MeanReversion(TrendType::Unknown),
                        format!("{}_MeanReversion", key),
                    ),
                    "trendfollow" => (
                        TradingStrategy::TrendFollow(TrendType::Unknown),
                        format!("{}_TrendFollow", key),
                    ),
                    &_ => panic!("Unknown strategy"),
                };

            let mut transaction_logs: Vec<TransactionLog> = Vec::new();
            for db_r_name in db_r_names.to_owned() {
                let log = TransactionLog::new(
                    Some(0),
                    Some(0),
                    Some(0),
                    &mongodb_uri,
                    &db_r_name,
                    &db_w_name,
                    false,
                )
                .await;
                transaction_logs.push(log);
            }

            let model_params = ModelParams::new(
                &mongodb_uri,
                &db_w_name,
                path_to_models.is_none(),
                path_to_models,
            )
            .await;

            let (x, y_classifier, y_regressor_1, y_regressor_2) =
                download_data(&transaction_logs, key, &strategy).await;

            grid_search_and_train_classifier(&file_key, &model_params, x.clone(), y_classifier, 5)
                .await;
            grid_search_and_train_regressor(
                &file_key,
                &model_params,
                x.clone(),
                y_regressor_1,
                5,
                30,
                1,
                Some(0.0),
            )
            .await;
            grid_search_and_train_regressor(
                &file_key,
                &model_params,
                x,
                y_regressor_2,
                5,
                30,
                2,
                Some(-1.0),
            )
            .await;
        }
        _ => {}
    }
    Ok(())
}

async fn run_default_program() -> std::io::Result<()> {
    // Load the configs
    let config = config::get_config_from_env().expect("Invalid configuration");

    // Set up the DB handler
    let max_position_counter = config.position_log_limit;
    let max_price_size = config.max_price_size * trade::TOKEN_LIST_SIZE;
    let db_handler = Arc::new(Mutex::new(
        DBHandler::new(
            max_position_counter,
            Some(max_price_size),
            Some(365),
            &config.mongodb_uri,
            &config.db_w_name,
            &config.db_r_name,
            config.back_test,
            config.path_to_models.as_ref(),
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

    let price_size = if config.back_test {
        None
    } else {
        Some(config.max_price_size)
    };
    let price_market_data = db_handler
        .lock()
        .await
        .get_latest_price_market_data(price_size)
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
    let (trading_interval, interval, dex_name) = &trader_config::get(&config.strategy)[0];

    // Create an error manager
    let error_manager = ErrorManager::new();

    let trader = DerivativeTrader::new(
        &dex_name,
        config.dry_run,
        *trading_interval,
        interval.clone(),
        config.interval_secs,
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
        &config.strategy,
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

        let invested_amount = trader.invested_amount();

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
                .log_app_state(
                    last_execution_time,
                    last_equity,
                    false,
                    None,
                    invested_amount,
                )
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

            // log the invested amount
            trader
                .db_handler()
                .lock()
                .await
                .log_app_state(None, None, false, None, invested_amount)
                .await;

            match trader.is_max_dd_occurred().await {
                Ok(is_dd) => {
                    if is_dd {
                        log::error!("Draw down!");
                        trader.liquidate(true, "Draw down").await;
                        trader
                            .db_handler()
                            .lock()
                            .await
                            .log_app_state(None, None, true, None, invested_amount)
                            .await;
                        log::info!("returned due to Draw down!");
                        error_manager.send("[debot] Draw down!", &config.db_w_name);
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
            result = trader_future => {
                match result {
                    Ok(_) => {
                        exit = false;
                    },
                    Err(_) => {
                        exit = true;
                    }
                }
            }
        }

        if exit {
            if config.liquidate_when_exit {
                trader.liquidate(true, "reboot").await;
            }
            std::process::exit(0);
        }

        let elapsed = loop_start.elapsed();
        let elapsed_millis = elapsed.as_millis() as u64;

        let max_elapsed = MAX_ELAPSED.load(Ordering::Relaxed);
        let elapsed_ave_millis = (max_elapsed + elapsed_millis) / 2;
        if elapsed_ave_millis > max_elapsed {
            log::warn!(
                "New max elapsed time: {:.1} s",
                elapsed_ave_millis as f64 / 1000.0
            );
            MAX_ELAPSED.store(elapsed_ave_millis, Ordering::Relaxed);
        }

        if elapsed.as_secs() > config.interval_secs.try_into().unwrap() {
            log::error!(
                "Elapsed time {} seconds exceeded the configured interval of {} seconds",
                elapsed.as_secs(),
                config.interval_secs
            );
        }

        let sleep_duration = if config.back_test {
            Duration::from_secs(0)
        } else {
            if let Some(remaining) =
                Duration::from_secs(config.interval_secs.try_into().unwrap()).checked_sub(elapsed)
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
) -> Result<(), ()> {
    let error_duration = Duration::from_secs(config.max_error_duration);
    let invested_amount = trader.invested_amount();

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
                invested_amount,
            )
            .await;
        error_manager.send("[debot] Continous error!", &config.db_w_name);
        return Err(());
    }

    match trader.find_chances().await {
        Ok(_) => {
            error_manager.reset_error_time();
        }
        Err(e) => {
            log::error!("Error while finding opportunities: {}", e);
            if let Some(io_error) = e.downcast_ref::<std::io::Error>() {
                if io_error.kind() == std::io::ErrorKind::InvalidData {
                    return Err(());
                }
            }
            error_manager.save_first_error_time();

            let _ = trader.reset_dex_client().await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{config::get_hyperliquid_config_from_env, trade::fund_config::TOKEN_LIST};
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
                        TOKEN_LIST,
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
