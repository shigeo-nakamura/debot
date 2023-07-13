// main.rs

use blockchain_factory::create_dexes;
use config::EnvConfig;
use db::create_unique_index;
use error_manager::ErrorManager;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::Address;
use ethers_middleware::providers::{Http, Provider};
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use shared_mongodb::ClientHolder;
use tokio::sync::Mutex;
use trade::transaction_log::get_last_transaction_id;
use trade::{ForcastTrader, PriceHistory, TransactionLog};

use crate::blockchain_factory::{create_base_token, create_tokens};
use crate::trade::{AbstractTrader, DBHandler, TraderState};
use crate::utils::ToDateTimeString;
use std::collections::HashMap;
use std::env;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use wallet::{create_wallet, get_balance_of_native_token};

mod addresses;
mod blockchain_factory;
mod config;
mod db;
mod dex;
mod error_manager;
mod kws_decrypt;
mod token;
mod trade;
mod utils;
mod wallet;

type WalletAndProvider = Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Just to satisfy Heroku
    let port = env::var("PORT").expect("PORT is not set");
    let _listener = TcpListener::bind(("0.0.0.0", port.parse().unwrap())).unwrap();

    // Load the configs
    let configs = config::get_config_from_env().expect("Invalid configuration");

    // Set up the DB client holder
    let mut client_options = match ClientOptions::parse(&configs[0].mongodb_uri).await {
        Ok(client_options) => client_options,
        Err(e) => {
            panic!("{:?}", e);
        }
    };
    let tls_options = TlsOptions::builder().build();
    client_options.tls = Some(Tls::Enabled(tls_options));
    let client_holder = Arc::new(Mutex::new(ClientHolder::new(client_options)));

    let db_name = &configs[0].db_name;
    let db = shared_mongodb::database::get(&client_holder, &db_name)
        .await
        .unwrap();
    create_unique_index(&db)
        .await
        .expect("Error creating unique index");

    // Set up the transaction log
    let last_position_counter = get_last_transaction_id(&db, db::CounterType::Position).await;
    let last_price_counter = get_last_transaction_id(&db, db::CounterType::Price).await;
    let last_performance_counter = get_last_transaction_id(&db, db::CounterType::Performance).await;

    let transaction_log = Arc::new(TransactionLog::new(
        configs[0].log_limit,
        last_position_counter,
        last_price_counter,
        last_performance_counter,
        &db_name,
    ));

    // Read the last App state
    let app_state = TransactionLog::get_app_state(&db).await;

    // Initialize an empty vector to hold trader instances
    let mut trader_instances = prepare_trader_instances(
        &configs,
        client_holder.clone(),
        transaction_log.clone(),
        app_state.prev_balance,
        app_state.trader_state,
    )
    .await;

    main_loop(
        &mut trader_instances,
        &configs,
        app_state.last_execution_time,
    )
    .await
}

async fn prepare_trader_instances(
    configs: &[EnvConfig],
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
    prev_balance: HashMap<String, Option<f64>>,
    trader_state: HashMap<String, TraderState>,
) -> Vec<(
    ForcastTrader,
    WalletAndProvider,
    Address,
    &EnvConfig,
    HashMap<String, PriceHistory>,
    ErrorManager,
)> {
    let mut trader_instances = Vec::new();

    for config in configs {
        let trader_instance = prepare_algorithm_trader_instance(
            config,
            client_holder.clone(),
            transaction_log.clone(),
            prev_balance.clone(),
            trader_state.clone(),
        )
        .await;
        trader_instances.push(trader_instance);
    }

    trader_instances
}

async fn prepare_algorithm_trader_instance(
    config: &EnvConfig,
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
    prev_balance: HashMap<String, Option<f64>>,
    trader_state: HashMap<String, TraderState>,
) -> (
    ForcastTrader,
    WalletAndProvider,
    Address,
    &EnvConfig,
    HashMap<String, PriceHistory>,
    ErrorManager,
) {
    // Create a wallet and provider
    let (wallet, wallet_and_provider) =
        create_wallet(&config.chain_params, config.rpc_node_index, config.use_kms)
            .await
            .unwrap();

    // Check the native token amount
    let gas_token_amount = get_balance_of_native_token(&config.chain_params, wallet.address())
        .await
        .unwrap();
    log::info!("gas token amount: {:3.3}", gas_token_amount);
    if !config.skip_write && gas_token_amount < config.chain_params.min_gas_token_amount {
        panic!("No enough gas token: {:3.3}", gas_token_amount);
    }

    // Create dexes
    let dexes = create_dexes(wallet_and_provider.clone(), &config.chain_params)
        .await
        .expect("Error creating DEXes");

    // Create Tokens
    let tokens = create_tokens(wallet_and_provider.clone(), &config.chain_params)
        .await
        .expect("Error creating tokens");

    // Create a base token
    let usdt_token = create_base_token(wallet_and_provider.clone(), &config.chain_params)
        .await
        .expect("Error creating a base token");

    // Create price histories
    let histories: HashMap<String, PriceHistory> = HashMap::new();

    // Create an error manager
    let error_manager = ErrorManager::new();

    // Read open positions from the DB
    let open_positions_map =
        DBHandler::get_open_positions_map(transaction_log.clone(), client_holder.clone()).await;

    // Read the last scores from the DB
    let scores = DBHandler::get_last_scores(transaction_log.clone(), client_holder.clone()).await;

    // Get the prev_balance
    let prev_balance = match prev_balance.get(config.chain_params.chain_name) {
        Some(balance) => *balance,
        None => None,
    };

    let mut trader = ForcastTrader::new(
        config.chain_params.chain_name,
        trader_state.clone(),
        config.leverage,
        config.min_managed_amount,
        config.min_trading_amount,
        config.allowance_factor,
        tokens.clone(),
        usdt_token.clone(),
        dexes.clone(),
        config.skip_write,
        config.chain_params.gas,
        config.short_trade_period,
        config.medium_trade_period,
        config.long_trade_period,
        config.max_price_size,
        config.flash_crash_threshold,
        config.position_creation_inteval_period * config.interval,
        config.reward_multiplier,
        config.penalty_multiplier,
        client_holder.clone(),
        transaction_log,
        config.dex_index,
        config.slippage,
        open_positions_map,
        prev_balance,
        scores,
    );

    trader.rebalance(wallet.address(), true).await;

    // Do some initialization
    trader
        .init(wallet.address(), config.min_managed_amount)
        .await
        .unwrap();

    (
        trader,
        wallet_and_provider,
        wallet.address(),
        config,
        histories,
        error_manager,
    )
}

async fn main_loop(
    trader_instances: &mut Vec<(
        ForcastTrader,
        WalletAndProvider,
        Address,
        &EnvConfig,
        HashMap<String, PriceHistory>,
        ErrorManager,
    )>,
    configs: &[EnvConfig],
    mut last_execution_time_map: HashMap<String, Option<SystemTime>>,
) -> std::io::Result<()> {
    let one_day = Duration::from_secs(24 * 60 * 60);
    let interval = configs[0].interval as f64 / trader_instances.len() as f64;

    for (_trader, _wallet_and_provider, _wallet_address, config, _histories, _error_manager) in
        trader_instances.iter_mut()
    {
        let last_execution_time = last_execution_time_map
            .get(config.chain_params.chain_name)
            .unwrap_or(&None)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        last_execution_time_map.insert(
            config.chain_params.chain_name.to_owned(),
            Some(last_execution_time),
        );

        log::warn!(
            "main_loop() starts for {}, last_execution_time = {}",
            config.chain_params.chain_name,
            last_execution_time.to_datetime_string()
        );
    }

    let mut paused = false;

    loop {
        let now = SystemTime::now();

        for (trader, wallet_and_provider, wallet_address, config, histories, error_manager) in
            trader_instances.iter_mut()
        {
            if trader.is_any_fund_liquidated() {
                paused = true;
            }

            if paused {
                trader.pause().await;
            }

            if trader.state() != TraderState::Active {
                continue;
            }

            let mut last_execution_time = last_execution_time_map
                .get(config.chain_params.chain_name)
                .unwrap()
                .unwrap();

            if now.duration_since(last_execution_time).unwrap() > one_day {
                let prev_balance = trader
                    .calculate_and_log_balance(config.chain_params.chain_name, wallet_address)
                    .await;
                last_execution_time = now;
                last_execution_time_map.insert(
                    config.chain_params.chain_name.to_owned(),
                    Some(last_execution_time),
                );
                trader
                    .db_handler()
                    .lock()
                    .await
                    .log_app_state(
                        Some(last_execution_time),
                        config.chain_params.chain_name,
                        prev_balance,
                        false,
                    )
                    .await;
            }

            if error_manager.get_error_count() >= config.max_error_count {
                log::error!("Error count reached the limit");
                trader.liquidate(config.chain_params.chain_name).await;
            }

            if let Some(_amount) = manage_token_amount(trader, &wallet_address, config).await {
                trader.rebalance(*wallet_address, true).await;
            }

            let mut opportunities = match trader.find_opportunities(histories).await {
                Ok(opportunities) => {
                    error_manager.reset_error_count();
                    opportunities
                }
                Err(e) => {
                    log::error!("Error while finding opportunities: {}", e);
                    error_manager.increment_error_count();
                    Vec::new()
                }
            };
            opportunities.sort_by(|a, b| {
                a.predicted_profit
                    .partial_cmp(&b.predicted_profit)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            match trader
                .execute_transactions(
                    &opportunities,
                    wallet_and_provider,
                    *wallet_address,
                    config.deadline_secs,
                )
                .await
            {
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

async fn manage_token_amount<T: AbstractTrader>(
    trader: &T,
    wallet_address: &Address,
    config: &EnvConfig,
) -> Option<f64> {
    if config.skip_write {
        return None;
    }

    let current_amount = match trader
        .get_amount_of_token(*wallet_address, &trader.base_token())
        .await
    {
        Ok(amount) => amount,
        Err(e) => {
            log::error!("get_current_token_amount: {:?}", e);
            return None;
        }
    };

    if config.treasury.is_some() && current_amount > config.max_managed_amount {
        let treasury = config.treasury.unwrap();
        let amount = current_amount - (config.max_managed_amount + config.min_managed_amount) / 2.0;
        match trader
            .transfer_token(treasury, &trader.base_token(), amount)
            .await
        {
            Ok(()) => {
                return Some(amount);
            }
            Err(e) => {
                log::error!("manage_token_amount transfer: {:?}", e);
                return None;
            }
        }
    }
    return None;
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
