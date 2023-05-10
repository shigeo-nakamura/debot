// main.rs

use crate::arbitrage::{Arbitrage, ArbitrageOpportunity, TwoTokenPairArbitrage};
use crate::dex::{ApeSwap, BakerySwap, BiSwap, Dex, PancakeSwap};
use crate::token::Token;
use crate::token_list::{create_tokens, create_usdt_token, BSC_CHAIN_PARAMS};
use ethers::providers::{Http, Provider};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{env, sync::RwLock};

mod addresses;
mod arbitrage;
mod dex;
mod http;
mod token;
mod token_list;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let provider_result = Provider::<Http>::try_from("https://bsc-dataseed.binance.org/");
    let provider = match provider_result {
        Ok(p) => p,
        Err(e) => {
            log::error!("Error creating provider: {:?}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Provider error",
            ));
        }
    };

    // Set up DEX list
    const DEX_LIST: &[&str] = &["PancakeSwap", "BiSwap" /*"BakerySwap", "ApeSwap" */];

    // Initialize DEX instances
    let dexes: Vec<(String, Box<dyn Dex>)> = DEX_LIST
        .iter()
        .map(|&dex_name| {
            let dex: Box<dyn Dex> = match dex_name {
                "PancakeSwap" => Box::new(PancakeSwap::new(provider.clone())),
                "BiSwap" => Box::new(BiSwap::new(provider.clone())),
                "BakerySwap" => Box::new(BakerySwap::new(provider.clone())),
                "ApeSwap" => Box::new(ApeSwap::new(provider.clone())),
                _ => panic!("Unknown DEX: {}", dex_name),
            };
            (dex_name.to_string(), dex)
        })
        .collect();
    let dexes = Arc::new(dexes);

    let interval_str = env::var("INTERVAL").unwrap_or_else(|_| "5".to_string());
    let interval = interval_str.parse::<u64>().unwrap();

    let amount_str = env::var("AMOUNT").unwrap_or_else(|_| "100.0".to_string());
    let amount = amount_str.parse::<f64>().unwrap();

    // Create the price history vector
    let price_history = Arc::new(RwLock::new(Vec::new()));

    // Start the HTTP server
    let server = http::start_server(price_history.clone()); // Use the start_server function from the http module
    actix_rt::spawn(server);

    loop {
        let start_instant = Instant::now();

        // Create Tokens
        let tokens = create_tokens(&vec![&BSC_CHAIN_PARAMS]);

        // Create a base token
        let usdt_token = create_usdt_token(&BSC_CHAIN_PARAMS).unwrap();

        // Clone tokens
        let tokens_cloned: Vec<Box<dyn Token>> = tokens.iter().map(|t| t.clone()).collect();

        // Create an instance of TwoTokenPairArbitrage
        let two_token_pair_arbitrage = TwoTokenPairArbitrage::new(amount, tokens, usdt_token);

        // Call the find_opportunities method for all tokens
        let opportunities_future =
            two_token_pair_arbitrage.find_opportunities(&dexes, price_history.clone());
        let ctrl_c_fut = tokio::signal::ctrl_c();

        // Run the tasks or break the loop if the ctrl_c signal is received
        let mut opportunities: Vec<ArbitrageOpportunity> = vec![];

        tokio::select! {
            result = opportunities_future => {
                log::info!("---------------------------------------------------------------");
                match result {
                    Ok(opportunities) => {

                        for opportunity in &opportunities {
                            let dex1 = &dexes[opportunity.dex1_index].1;
                            let dex2 = &dexes[opportunity.dex2_index].1;
                            let token_a = &tokens_cloned[opportunity.token_a_index];
                            let token_b = &tokens_cloned[opportunity.token_b_index];                        }
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
