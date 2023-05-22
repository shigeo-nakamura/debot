// main.rs

use config::EnvConfig;
use ethers::signers::Signer;
use http::TransactionResult;
use rand::Rng;
use token_manager::create_dexes;

use crate::arbitrage::{Arbitrage, TwoTokenPairArbitrage};
use crate::token_manager::{create_base_token, create_tokens};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use wallet::{create_kms_wallet, create_local_wallet};

mod addresses;
mod arbitrage;
mod config;
mod dex;
mod http;
mod token;
mod token_manager;
mod wallet;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Load the configs
    let configs = config::get_config_from_env().expect("Invalid configuration");

    // Create the transaction result vector
    let transaction_result = Arc::new(RwLock::new(Vec::new()));

    // Start the HTTP server
    setup_server(transaction_result.clone());

    loop {
        let mut found_opportunity = false;

        for config in &configs {
            let result = try_arbitrage(config, transaction_result.clone()).await;
            match result {
                Ok(has_opportunity) => {
                    if has_opportunity {
                        found_opportunity = true;
                    }
                }
                Err(e) => {
                    log::error!("Error occurred during arbitrage: {:?}", e);
                }
            }
        }

        if !found_opportunity {
            let mut rng = rand::thread_rng();
            let sleep_secs: u64 = rng.gen_range(0..configs[0].interval);
            let sleep_fut = tokio::time::sleep(Duration::from_secs(sleep_secs));
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

fn setup_server(
    transaction_result: Arc<RwLock<Vec<TransactionResult>>>,
) -> tokio::task::JoinHandle<std::result::Result<(), std::io::Error>> {
    let server = http::start_server(transaction_result);
    actix_rt::spawn(server)
}

async fn try_arbitrage(
    config: &EnvConfig,
    transaction_result: Arc<RwLock<Vec<TransactionResult>>>,
) -> std::io::Result<bool> {
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

    // Create an instance of TwoTokenPairArbitrage
    let two_token_pair_arbitrage = TwoTokenPairArbitrage::new(
        config.amount,
        config.allowance_factor,
        tokens.clone(),
        usdt_token.clone(),
        dexes.clone(),
        config.skip_write,
    );

    two_token_pair_arbitrage
        .init(wallet.address())
        .await
        .unwrap();

    // Call the find_opportunities method for all tokens
    let opportunities_future = two_token_pair_arbitrage.find_opportunities();
    let ctrl_c_fut = tokio::signal::ctrl_c();

    let opportunities = tokio::select! {
        result = opportunities_future => {
            match result {
                Ok(opportunities) => {
                    for opportunity in &opportunities {
                        let _result = two_token_pair_arbitrage.execute_transactions(&opportunity, &wallet_and_provider, wallet.address(), config.deadline_secs, transaction_result.clone(), config.log_limit).await.unwrap_or_else(|err| {
                            log::error!("Error occurred: {:?}", err);
                        });
                    }
                    opportunities
                },
                Err(e) => {
                    let err_msg = format!("Error while finding opportunities: {}", e);
                    log::error!("{}", err_msg);
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, err_msg));
                }
            }
        },
        _ = ctrl_c_fut => {
            log::info!("SIGINT received. Shutting down...");
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "SIGINT received. Shutting down..."));
        }
    };

    Ok(!opportunities.is_empty())
}
