// main.rs

use arbitrage::{ArbitrageOpportunity, PriceHistory, ReversionArbitrage};
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

        // Create price histories
        let histories: HashMap<String, PriceHistory> = HashMap::new();

        arbitrage.init(wallet.address()).await.unwrap();

        // Push each Arbitrage instance and its associated wallet_and_provider into the vector
        arbitrage_instances.push((
            arbitrage,
            wallet_and_provider,
            wallet.address(),
            config,
            histories,
        ));
    }

    loop {
        log::info!("### enter");
        for (arbitrage, wallet_and_provider, wallet_address, config, histories) in
            arbitrage_instances.iter_mut()
        {
            let mut opportunities = arbitrage
                .find_opportunities(histories)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Error while finding opportunities: {}", e);
                    Vec::new()
                });
            if opportunities.is_empty() {
                continue;
            }

            opportunities.sort_by(|a, b| {
                a.profit
                    .partial_cmp(&b.profit)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut profitable_opportunities: Vec<ArbitrageOpportunity> = vec![];
            for opportunity in opportunities {
                opportunity.print_info(&arbitrage.dexes(), &arbitrage.tokens());
                profitable_opportunities.push(opportunity);
            }
            if profitable_opportunities.is_empty() {
                log::info!(".");
            } else {
                arbitrage
                    .execute_transactions(
                        &profitable_opportunities,
                        wallet_and_provider,
                        *wallet_address,
                        config.deadline_secs,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("Error while executing transactions: {}", e);
                    });
            }
        }
        log::info!("### leave");

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
