// main.rs

use blockchain_factory::create_dexes;
use chrono::{DateTime, Utc};
use config::EnvConfig;
use error_manager::ErrorManager;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::Address;
use ethers_middleware::providers::{Http, Provider};
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use shared_mongodb::ClientHolder;
use tokio::sync::Mutex;
use trade::transaction_log::{get_last_transaction_id, AppState};
use trade::{ForcastTrader, PriceHistory, TransactionLog};

use crate::blockchain_factory::{create_base_token, create_tokens};
use crate::trade::AbstractTrader;
use std::cmp::max;
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

    // Set up the transaction log
    let db_name = &configs[0].db_name;
    let db = shared_mongodb::database::get(&client_holder, &db_name)
        .await
        .unwrap();
    let last_transaction_id = get_last_transaction_id(&db).await;
    let transaction_log = Arc::new(TransactionLog::new(
        configs[0].log_limit,
        last_transaction_id,
        &db_name,
    ));

    // Read the last App state
    let db = transaction_log
        .get_db(&client_holder.clone())
        .await
        .unwrap();
    let app_state = TransactionLog::get_app_state(&db).await;

    // Initialize an empty vector to hold trader instances
    let mut trader_instances = prepare_trader_instances(
        &configs,
        client_holder.clone(),
        transaction_log.clone(),
        app_state.prev_balance,
    )
    .await;

    main_loop(
        &mut trader_instances,
        &configs,
        app_state.last_execution_time,
        client_holder,
        transaction_log.clone(),
    )
    .await
}

async fn prepare_trader_instances(
    configs: &[EnvConfig],
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
    prev_balance: Option<f64>,
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
            prev_balance,
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
    prev_balance: Option<f64>,
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
        ForcastTrader::get_open_positions_map(transaction_log.clone(), client_holder.clone()).await;

    let mut trader = ForcastTrader::new(
        config.leverage,
        config.min_managed_amount,
        config.allowance_factor,
        tokens.clone(),
        usdt_token.clone(),
        dexes.clone(),
        config.skip_write,
        config.chain_params.gas,
        config.short_trade_period,
        config.medium_trade_period,
        config.long_trade_period,
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
    );

    trader.rebalance(wallet.address()).await;

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
    last_execution_time: SystemTime,
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
) -> std::io::Result<()> {
    let now = SystemTime::now();
    let one_day = Duration::from_secs(24 * 60 * 60);
    let mut last_execution_time = last_execution_time;

    let datetime: DateTime<Utc> = last_execution_time.into();

    log::warn!(
        "main_loop() starts, last_execution_time = {}",
        datetime.format("%Y-%m-%d %H:%M:%S")
    );

    loop {
        let interval = configs[0].interval as f64 / trader_instances.len() as f64;

        for (trader, wallet_and_provider, wallet_address, config, histories, error_manager) in
            trader_instances.iter_mut()
        {
            if now.duration_since(last_execution_time).unwrap() > one_day {
                let prev_balance = trader.log_current_balance(wallet_address).await;
                last_execution_time = now;
                let db = transaction_log
                    .get_db(&client_holder.clone())
                    .await
                    .unwrap();

                TransactionLog::update_app_state(&db, last_execution_time, prev_balance);
            }

            if error_manager.get_error_count() >= config.max_error_count {
                log::error!("Error count reached the limit");
                trader.close_all_positions();
            }

            if let Some(_amount) = manage_token_amount(trader, &wallet_address, config).await {
                trader.rebalance(*wallet_address).await;
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
            if trader.is_close_all_positions() {
                continue;
            }

            if let Err(e) = handle_sleep_and_signal(interval).await {
                log::error!("Error handling sleep and signal: {}", e);
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
    let ctrl_c_fut = tokio::signal::ctrl_c();
    tokio::select! {
        _ = sleep_fut => {
            // continue to the next iteration of loop
        },
        _ = ctrl_c_fut => {
            log::info!("SIGINT received. Shutting down...");
            return Err("SIGINT received");
        }
    }
    Ok(())
}
