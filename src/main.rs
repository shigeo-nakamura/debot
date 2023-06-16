// main.rs

use arbitrage::{ArbitrageOpportunity, PriceHistory, ReversionArbitrage};
use error_manager::ErrorManager;
use ethers::signers::Signer;
use token_manager::create_dexes;

use crate::arbitrage::Arbitrage;
use crate::token_manager::{create_base_token, create_tokens};
use std::collections::HashMap;
use std::time::Duration;
use wallet::create_wallet;

mod addresses;
mod arbitrage;
mod config;
mod dex;
mod error_manager;
mod kws_decrypt;
mod token;
mod token_manager;
mod wallet;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Load the configs
    let configs = config::get_config_from_env().expect("Invalid configuration");

    // Initialize an empty vector to hold your arbitrage instances
    let mut arbitrage_instances = Vec::new();

    for config in &configs {
        // Create a wallet and provider
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

        // Create an instance of Arbitrage
        let arbitrage = ReversionArbitrage::new(
            config.amount,
            config.allowance_factor,
            tokens.clone(),
            usdt_token.clone(),
            dexes.clone(),
            config.skip_write,
            config.chain_params.gas,
            config.short_trade_period,
            config.medium_trade_period,
            config.long_trade_period,
            config.percentage_loss_threshold,
            config.percentage_profit_threshold,
            config.percentage_drop_threshold,
            config.max_position_amount,
            config.max_hold_period,
            config.match_multiplier,
            config.mismatch_multiplier,
            config.log_limit,
        );

        // Create an error manager
        let error_manager = ErrorManager::new();

        // Do some initialization
        arbitrage
            .init(wallet.address(), config.min_initial_amount)
            .await
            .unwrap();

        // Push each Arbitrage instance and its associated wallet_and_provider into the vector
        arbitrage_instances.push((
            arbitrage,
            wallet_and_provider,
            wallet.address(),
            config,
            histories,
            error_manager,
        ));
    }

    loop {
        let mut skip_sleep = false;
        log::info!("### enter");
        for (arbitrage, wallet_and_provider, wallet_address, config, histories, error_maanger) in
            arbitrage_instances.iter_mut()
        {
            if error_maanger.get_error_count() >= config.max_error_count {
                log::error!("Error count reached the limit");
                arbitrage.close_all_positions();
            }

            let mut opportunities = arbitrage
                .find_opportunities(histories)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Error while finding opportunities: {}", e);
                    error_maanger.increment_error_count();
                    Vec::new()
                });
            if opportunities.is_empty() {
                continue;
            }

            opportunities.sort_by(|a, b| {
                a.predicted_profit
                    .partial_cmp(&b.predicted_profit)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            arbitrage
                .execute_transactions(
                    &opportunities,
                    wallet_and_provider,
                    *wallet_address,
                    config.deadline_secs,
                )
                .await
                .unwrap_or_else(|e| {
                    log::error!("Error while executing transactions: {}", e);
                    error_maanger.increment_error_count();
                });
            if arbitrage.is_close_all_positions() {
                skip_sleep = true;
            }
        }
        log::info!("### leave");

        if !skip_sleep {
            let sleep_fut = tokio::time::sleep(Duration::from_secs(configs[0].interval));
            let ctrl_c_fut = tokio::signal::ctrl_c();
            tokio::select! {
                _ = sleep_fut => {
                    // continue to the next iteration of loop
                },
                _ = ctrl_c_fut => {
                    log::info!("SIGINT received. Shutting down...");
                    return Ok(());
                }
            }
        }
    }
}
