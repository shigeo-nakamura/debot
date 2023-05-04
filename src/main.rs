use ethers::{
    providers::{Http, Provider},
    types::Address,
};
use futures::{stream::FuturesUnordered, StreamExt};
use std::time::{Duration, Instant, SystemTime};
use std::{env, sync::RwLock};
use std::{str::FromStr, sync::Arc};

use crate::addresses::{
    BNB_ADDRESS, BTCB_ADDRESS, BUSD_ADDRESS, ETH_ADDRESS, USDC_ADDRESS,
    USDT_ADDRESS,
    /* CAKE_ADDRESS, TUSD_ADDRESS, */
};
use crate::dex::{ApeSwap, BakerySwap, BiSwap, Dex, PancakeSwap};

mod addresses;
mod arbitrage;
mod dex;
mod http;

use http::PriceData;

async fn calculate_usdt_arbitrage_profit(
    dex1: Box<dyn Dex + Send + Sync>,
    dex2: Box<dyn Dex + Send + Sync>,
    token_a: Address,
    amount: f64,
    token_symbol: &str,
    price_history: Arc<RwLock<Vec<PriceData>>>,
) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let usdt_address = Address::from_str(USDT_ADDRESS).unwrap();

    let swap_to_token_price = dex1.get_token_price(usdt_address, token_a, amount).await?;
    let token_b_amount = amount * swap_to_token_price;

    log::debug!(
        "Dex 1: {}, USDT --> {}, Input Amount: {}, Output Amount: {}, price: {}",
        dex1.get_name(),
        token_symbol,
        amount,
        token_b_amount,
        swap_to_token_price,
    );

    let swap_to_usdt_price = dex2
        .get_token_price(token_a, usdt_address, token_b_amount)
        .await?;
    let final_usdt_amount = token_b_amount * swap_to_usdt_price;

    log::debug!(
        "Dex 2 {}, {} --> USDT, Input Amount: {}, Output Amount: {}, price: {}",
        dex2.get_name(),
        token_symbol,
        token_b_amount,
        final_usdt_amount,
        swap_to_usdt_price
    );

    let profit = final_usdt_amount - amount;
    if profit > 0.0 {
        log::info!(
            "Arbitrage opportunity [{} and {}] for (USDT - {}). Profit: {} USDT",
            dex1.get_name(),
            dex2.get_name(),
            token_symbol,
            profit
        );
    } else {
        log::info!(
            "No arbitrage opportunity [{} and {}] for (USDT - {}). Loss: {} USDT",
            dex1.get_name(),
            dex2.get_name(),
            token_symbol,
            profit
        );
    }

    // Store price data in price_history
    store_price_history(
        dex1.get_name(),
        dex2.get_name(),
        token_symbol,
        swap_to_token_price,
        swap_to_usdt_price,
        profit,
        price_history,
    );

    Ok(profit)
}

fn store_price_history(
    dex1_name: &str,
    dex2_name: &str,
    token_symbol: &str,
    swap_to_token_price: f64,
    swap_to_usdt_price: f64,
    profit: f64,
    price_history: Arc<RwLock<Vec<PriceData>>>,
) {
    let mut price_history_guard = price_history.write().unwrap();
    price_history_guard.push(PriceData {
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        tokens: [String::from("USDT"), String::from(token_symbol)].to_vec(),
        dex_prices: vec![
            (String::from(dex1_name), swap_to_token_price),
            (String::from(dex2_name), swap_to_usdt_price),
        ],
        profit: profit,
    });
}

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

    const TOKEN_PAIRS: &[(&str, &str)] = &[
        ("BNB ", BNB_ADDRESS),
        ("BTCB", BTCB_ADDRESS),
        ("ETH ", ETH_ADDRESS),
        ("BUSD", BUSD_ADDRESS),
        ("USDC", USDC_ADDRESS),
        //("CAKE", CAKE_ADDRESS),
        //("TUSD", TUSD_ADDRESS),
    ];

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

        let dexes_clone = dexes.clone();
        let tasks: Vec<_> = TOKEN_PAIRS
            .iter()
            .map(|&(token_a_symbol, token_a_address_str)| {
                let token_a_address = Address::from_str(token_a_address_str).unwrap();
                let boxed_dexes = dexes_clone
                    .iter()
                    .map(|(dex_name, dex)| (dex_name.clone(), dex.clone_box()))
                    .collect::<Vec<_>>();

                // Pass the price_history to the calculate_usdt_arbitrage_profit function
                let price_history_clone = price_history.clone();
                tokio::spawn(async move {
                    let mut profitable_dex_pairs = Vec::new();
                    for i in 0..boxed_dexes.len() {
                        for j in 0..boxed_dexes.len() {
                            if i != j {
                                let estimate_profit = calculate_usdt_arbitrage_profit(
                                    boxed_dexes[i].1.clone_box(),
                                    boxed_dexes[j].1.clone_box(),
                                    token_a_address,
                                    amount,
                                    token_a_symbol,
                                    price_history_clone.clone(), // Pass the price_history Arc<RwLock> to the function
                                )
                                .await;
                                if let Ok(profit) = estimate_profit {
                                    if profit > 0.0 {
                                        profitable_dex_pairs.push((i, j, profit));
                                    }
                                }
                            }
                        }
                    }

                    if !profitable_dex_pairs.is_empty() {
                        log::info!(
                            "Profitable opportunities for {} ({}):",
                            token_a_symbol,
                            token_a_address
                        );
                        for (i, j, profit) in profitable_dex_pairs {
                            log::info!(
                                "  Buy on {} and sell on {}: {:?}",
                                boxed_dexes[i].0,
                                boxed_dexes[j].0,
                                profit
                            );
                        }
                    }
                })
            })
            .collect();

        let tasks_future = FuturesUnordered::from_iter(tasks);

        let ctrl_c_fut = tokio::signal::ctrl_c();

        // Run the tasks or break the loop if the ctrl_c signal is received
        tokio::select! {
            _ = tasks_future.collect::<Vec<_>>() => {
                log::info!("---------------------------------------------------------------");
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
