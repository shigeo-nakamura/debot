// main.rs

use blockchain_factory::create_dexes;
use config::EnvConfig;
use error_manager::ErrorManager;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::Address;
use ethers_middleware::providers::{Http, Provider};
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use trade::{ForcastTrader, PriceHistory, TradingStrategy};

use crate::blockchain_factory::{create_base_token, create_tokens};
use crate::trade::AbstractTrader;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use wallet::create_wallet;

mod addresses;
mod blockchain_factory;
mod config;
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

    // Load the configs
    let configs = config::get_config_from_env().expect("Invalid configuration");

    // Initialize an empty vector to hold trader instances
    let mut trader_instances = prepare_trader_instances(&configs).await;

    main_loop(&mut trader_instances, &configs).await
}

async fn prepare_trader_instances(
    configs: &[EnvConfig],
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
        let trader_instance = prepare_algorithm_trader_instance(config).await;
        trader_instances.push(trader_instance);
    }

    trader_instances
}

async fn prepare_algorithm_trader_instance(
    config: &EnvConfig,
) -> (
    ForcastTrader,
    WalletAndProvider,
    Address,
    &EnvConfig,
    HashMap<String, PriceHistory>,
    ErrorManager,
) {
    let strategies = vec![
        TradingStrategy::TrendFollowing,
        TradingStrategy::MeanReversion,
        TradingStrategy::Contrarian,
    ];

    let (wallet, wallet_and_provider) = create_wallet(&config.chain_params, config.use_kms)
        .await
        .unwrap();

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
        config.take_profit_threshold,
        config.cut_loss_threshold,
        config.flash_crash_threshold,
        config.max_hold_interval,
        config.position_creation_inteval_period * config.interval,
        config.reward_multiplier,
        config.penalty_multiplier,
        config.log_limit,
        config.initial_score,
    );

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
) -> std::io::Result<()> {
    loop {
        let mut skip_sleep = false;
        log::info!("### enter");
        for (trader, wallet_and_provider, wallet_address, config, histories, error_manager) in
            trader_instances.iter_mut()
        {
            if error_manager.get_error_count() >= config.max_error_count {
                log::error!("Error count reached the limit");
                trader.close_all_positions();
            }

            manage_token_amount(trader, wallet_address, config).await?;

            let mut opportunities =
                trader
                    .find_opportunities(histories)
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Error while finding opportunities: {}", e);
                        error_manager.increment_error_count();
                        Vec::new()
                    });
            opportunities.sort_by(|a, b| {
                a.predicted_profit
                    .partial_cmp(&b.predicted_profit)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            trader
                .execute_transactions(
                    &opportunities,
                    wallet_and_provider,
                    *wallet_address,
                    config.deadline_secs,
                )
                .await
                .unwrap_or_else(|e| {
                    log::error!("Error while executing transactions: {}", e);
                    error_manager.increment_error_count();
                });
            skip_sleep = trader.is_close_all_positions();
        }
        log::info!("### leave");

        if let Err(e) = handle_sleep_and_signal(skip_sleep, configs[0].interval).await {
            log::error!("Error handling sleep and signal: {}", e);
            return Ok(());
        }
    }
}

async fn manage_token_amount<T: AbstractTrader>(
    trader: &mut T,
    wallet_address: &Address,
    config: &EnvConfig,
) -> std::io::Result<()> {
    let current_amount = match trader
        .get_amount_of_token(*wallet_address, &trader.base_token())
        .await
    {
        Ok(amount) => amount,
        Err(e) => {
            log::error!("{:?}", e);
            return Ok(());
        }
    };

    if current_amount > config.max_managed_amount {
        let amount = current_amount - config.min_managed_amount;
        match trader
            .transfer_token(*wallet_address, &trader.base_token(), amount)
            .await
        {
            Ok(()) => {}
            Err(e) => {
                log::error!("{:?}", e);
                return Ok(());
            }
        }
    }
    Ok(())
}

async fn handle_sleep_and_signal(skip_sleep: bool, interval: u64) -> Result<(), &'static str> {
    if !skip_sleep {
        let sleep_fut = tokio::time::sleep(Duration::from_secs(interval));
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
    }
    Ok(())
}
