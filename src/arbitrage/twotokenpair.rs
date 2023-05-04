use ethers::{
    providers::{Http, Provider},
    types::Address,
};
use std::time::{Duration, Instant, SystemTime};
use std::{env, sync::RwLock};
use std::{str::FromStr, sync::Arc};
use crate::addresses::{
    BNB_ADDRESS, BTCB_ADDRESS, BUSD_ADDRESS, CAKE_ADDRESS, ETH_ADDRESS, TUSD_ADDRESS, USDC_ADDRESS,
    USDT_ADDRESS,
};
use crate::{
    addresses::BAKERY_SWAP_ROUTER,
    dex::{ApeSwap, BakerySwap, BiSwap, Dex, PancakeSwap},
};
use crate::http::PriceData;

pub struct TwoTokenPairArbitrage {
   pub amount: f64,
}

    const TOKEN_PAIRS: &[(&str, &str)] = &[
        ("BNB", BNB_ADDRESS),
        ("BTCB", BTCB_ADDRESS),
        ("ETH", ETH_ADDRESS),
        ("BUSD", BUSD_ADDRESS),
        ("USDC", USDC_ADDRESS),
        //("CAKE", CAKE_ADDRESS),
        //("TUSD", TUSD_ADDRESS),
    ];

    impl Arbitrage for TwoTokenPairArbitrage {
        async fn find_opportunities(
            &self,
            dexes: &[(String, Box<dyn Dex>)],
            price_history: Arc<RwLock<Vec<PriceData>>>,
        ) -> Vec<ArbitrageOpportunity> {
            let mut opportunities: Vec<ArbitrageOpportunity> = vec![];
    
            // Loop through each token in TOKEN_PAIRS
            for (token_symbol, token_address) in TOKEN_PAIRS.iter() {
                let token_a = Address::from_str(token_address).unwrap();
    
                // Loop through all combinations of dexes
                for (dex1_index, (dex1_name, dex1)) in dexes.iter().enumerate() {
                    for (dex2_index, (dex2_name, dex2)) in dexes.iter().enumerate() {
                        if dex1_index == dex2_index {
                            continue;
                        }
    
                        let usdt_address = Address::from_str(USDT_ADDRESS).unwrap();
                        let swap_to_token_price = dex1.get_token_price(usdt_address, token_a, self.amount).await?;
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
                                "Arbitrage opportunity detected between {} and {} for USDT - {}! Profit: {} USDT",
                                dex1.get_name(),
                                dex2.get_name(),
                                token_symbol,
                                profit
                            );
                            opportunities.push(ArbitrageOpportunity {
                                // Fill the struct with relevant information
                            });
                        } else {
                            log::info!(
                                "No arbitrage opportunity detected between {} and {} for USDT - {}. Loss: {} USDT",
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
                            price_history.clone(),
                        );
                    }
                }
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

            // ... (continue with the rest of the find_opportunities() method)

            // You may need to return a vector of ArbitrageOpportunity structs based on the calculated profit.
            // Create the struct and add it to the vector if there's an arbitrage opportunity.
            // Return the vector of arbitrage opportunities at the end.

            opportunities
        }
    }
