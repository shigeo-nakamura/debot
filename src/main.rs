// main.rs

use arbitrage::ArbitrageOpportunity;
use config::EnvConfig;
use ethers::signers::Signer;
use rand::Rng;
use token_manager::create_dexes;

use crate::arbitrage::{Arbitrage, TriangleArbitrage};
use crate::token_manager::{create_base_token, create_tokens};
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use wallet::{create_kms_wallet, create_local_wallet};

mod addresses;
mod arbitrage;
mod config;
mod dex;
mod token;
mod token_manager;
mod wallet;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Load the configs
    let configs = config::get_config_from_env().expect("Invalid configuration");

    // Initialize an empty vector to hold your arbitrage instances
    let mut arbitrage_instances = Vec::new();

    for config in &configs {
        // Create a wallet and provider
        let (wallet, wallet_and_provider) = create_local_wallet(&config.chain_params).unwrap();

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

        // Create an instance of TriangleArbitrage
        let triangle_arbitrage = TriangleArbitrage::new(
            config.amount,
            config.allowance_factor,
            tokens.clone(),
            usdt_token.clone(),
            dexes.clone(),
            config.skip_write,
            config.num_swaps,
            config.chain_params.gas,
        );

        triangle_arbitrage.init(wallet.address()).await.unwrap();

        let paths = triangle_arbitrage.find_arbitrage_paths().unwrap();

        // Push each TriangleArbitrage instance and its associated wallet_and_provider into the vector
        arbitrage_instances.push((
            triangle_arbitrage,
            wallet_and_provider,
            wallet.address(),
            config,
            paths,
        ));
    }

    loop {
        log::info!("### enter");
        for (triangle_arbitrage, wallet_and_provider, wallet_address, config, paths) in
            &arbitrage_instances
        {
            let mut opportunities = triangle_arbitrage
                .find_path_opportunities(paths)
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
                opportunity.print_info(&triangle_arbitrage.dexes(), &triangle_arbitrage.tokens());
                if opportunity.profit > 0.0 {
                    profitable_opportunities.push(opportunity);
                }
            }

            if profitable_opportunities.is_empty() {
                log::info!(".");
            } else {
                triangle_arbitrage
                    .execute_transactions(
                        &profitable_opportunities,
                        wallet_and_provider,
                        *wallet_address,
                        config.deadline_secs,
                        config.log_limit,
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
