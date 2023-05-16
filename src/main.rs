// main.rs

use ethers::signers::Signer;
use token_manager::create_dexes;

use crate::arbitrage::{Arbitrage, TwoTokenPairArbitrage};
use crate::token_manager::{create_tokens, create_usdt_token, create_wallet, BSC_CHAIN_PARAMS};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{env, sync::RwLock};

mod addresses;
mod arbitrage;
mod config;
mod dex;
mod http;
mod token;
mod token_manager;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let config = config::get_config_from_env().expect("Invalid configuration");

    // Create a wallet
    let wallet = create_wallet().unwrap();

    // Create dexes
    let dexes = create_dexes(config.chain_params).expect("Error creating DEXes");

    // Create Tokens
    let tokens =
        create_tokens(config.chain_params, wallet.clone()).expect("Error creating Ttokens");

    // Create a base token
    let usdt_token =
        create_usdt_token(&BSC_CHAIN_PARAMS, wallet.clone()).expect("Error creating a base token");

    // Create an instance of TwoTokenPairArbitrage
    let two_token_pair_arbitrage = TwoTokenPairArbitrage::new(
        config.amount,
        tokens.clone(),
        usdt_token.clone(),
        dexes.clone(),
    );
    two_token_pair_arbitrage
        .init(wallet.address())
        .await
        .unwrap();

    // Create the price history vector
    let price_history = Arc::new(RwLock::new(Vec::new()));

    // Start the HTTP server
    let server = http::start_server(price_history.clone()); // Use the start_server function from the http module
    actix_rt::spawn(server);

    loop {
        let start_instant = Instant::now();

        // Call the find_opportunities method for all tokens
        let opportunities_future =
            two_token_pair_arbitrage.find_opportunities(price_history.clone());
        let ctrl_c_fut = tokio::signal::ctrl_c();

        tokio::select! {
            result = opportunities_future => {
                match result {
                    Ok(opportunities) => {
                        for opportunity in &opportunities {
                            two_token_pair_arbitrage.execute_transactions(&opportunity, &wallet).await.unwrap();
                        }
                    },
                    Err(e) => {
                        log::error!("Error while finding opportunities: {}", e);
                    }
                }
            },
            _ = ctrl_c_fut => {
                println!("SIGINT received. Shutting down...");
                break Ok(());
            }
        }

        let elapsed = start_instant.elapsed();
        if elapsed < Duration::from_secs(config.interval) {
            tokio::time::sleep(Duration::from_secs(config.interval) - elapsed).await;
        }
    }
}
