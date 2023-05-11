// main.rs

use ethers::signers::Signer;
use token_manager::create_dexes;

use crate::arbitrage::{Arbitrage, ArbitrageOpportunity, TwoTokenPairArbitrage};
use crate::dex::{ApeSwap, BakerySwap, BiSwap, Dex, PancakeSwap};
use crate::token::Token;
use crate::token_manager::{
    create_provider, create_tokens, create_usdt_token, create_wallet, BSC_CHAIN_PARAMS,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{env, sync::RwLock};

mod addresses;
mod arbitrage;
mod dex;
mod http;
mod token;
mod token_manager;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let interval_str = env::var("INTERVAL").unwrap_or_else(|_| "5".to_string());
    let interval = interval_str.parse::<u64>().unwrap();

    let amount_str = env::var("AMOUNT").unwrap_or_else(|_| "100.0".to_string());
    let amount = amount_str.parse::<f64>().unwrap();

    // Create a wallet
    let wallet = create_wallet().unwrap();

    // Create dexes
    let dexes = create_dexes(&BSC_CHAIN_PARAMS).expect("Error creating DEXes");

    // Create the price history vector
    let price_history = Arc::new(RwLock::new(Vec::new()));

    // Start the HTTP server
    let server = http::start_server(price_history.clone()); // Use the start_server function from the http module
    actix_rt::spawn(server);

    loop {
        let start_instant = Instant::now();

        // Create Tokens
        let tokens = create_tokens(&BSC_CHAIN_PARAMS, wallet.clone()).unwrap();

        // Create a base token
        let usdt_token = create_usdt_token(&BSC_CHAIN_PARAMS, wallet.clone()).unwrap();

        // Create an instance of TwoTokenPairArbitrage
        let two_token_pair_arbitrage = TwoTokenPairArbitrage::new(amount, tokens, usdt_token);
        let spender_address = wallet.address();
        two_token_pair_arbitrage
            .init(spender_address)
            .await
            .unwrap();

        // Call the find_opportunities method for all tokens
        let opportunities_future =
            two_token_pair_arbitrage.find_opportunities(&dexes, price_history.clone());
        let ctrl_c_fut = tokio::signal::ctrl_c();

        tokio::select! {
            result = opportunities_future => {
                log::info!("---------------------------------------------------------------");
                match result {
                    Ok(opportunities) => {
                        for opportunity in &opportunities {
                            two_token_pair_arbitrage.execute_transactions(&opportunity, &dexes, &wallet).await.unwrap();
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
        if elapsed < Duration::from_secs(interval) {
            tokio::time::sleep(Duration::from_secs(interval) - elapsed).await;
        }
    }
}
