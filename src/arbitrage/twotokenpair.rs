use crate::addresses::{
    BNB_ADDRESS, BTCB_ADDRESS, BUSD_ADDRESS, ETH_ADDRESS, USDC_ADDRESS, USDT_ADDRESS,
};
use crate::dex::Dex;
use crate::http::PriceData;
use anyhow::{Context, Result};
use async_trait::async_trait;
use ethers::types::Address;
use std::sync::RwLock;
use std::{str::FromStr, sync::Arc};

use super::{Arbitrage, ArbitrageOpportunity};

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

#[async_trait]
impl Arbitrage for TwoTokenPairArbitrage {
    async fn find_opportunities(
        &self,
        dexes: &[(String, Box<dyn Dex>)],
        price_history: Arc<RwLock<Vec<PriceData>>>,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities: Vec<ArbitrageOpportunity> = vec![];

        // Loop through each token in TOKEN_PAIRS
        for (token_symbol, token_address) in TOKEN_PAIRS.iter() {
            let token_a = Address::from_str(token_address).unwrap();

            // Loop through all combinations of dexes
            for (dex1_index, (_dex1_name, dex1)) in dexes.iter().enumerate() {
                for (dex2_index, (_dex2_name, dex2)) in dexes.iter().enumerate() {
                    if dex1_index == dex2_index {
                        continue;
                    }

                    let usdt_address = Address::from_str(USDT_ADDRESS).unwrap();
                    let swap_to_token_price = dex1
                        .get_token_price(usdt_address, token_a, self.amount)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))
                        .context("Error getting token price from dex1")?;
                    let token_b_amount = self.amount * swap_to_token_price;

                    log::debug!(
                        "Dex 1: {}, USDT --> {}, Input Amount: {}, Output Amount: {}, price: {}",
                        dex1.get_name(),
                        token_symbol,
                        self.amount,
                        token_b_amount,
                        swap_to_token_price,
                    );

                    let swap_to_usdt_price = dex2
                        .get_token_price(token_a, usdt_address, token_b_amount)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))
                        .context("Error getting token price from dex2")?;
                    let final_usdt_amount = token_b_amount * swap_to_usdt_price;

                    log::debug!(
                        "Dex 2 {}, {} --> USDT, Input Amount: {}, Output Amount: {}, price: {}",
                        dex2.get_name(),
                        token_symbol,
                        token_b_amount,
                        final_usdt_amount,
                        swap_to_usdt_price
                    );

                    let profit = final_usdt_amount - self.amount;
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
                    TwoTokenPairArbitrage::store_price_history(
                        &[dex1.get_name(), dex2.get_name()],
                        &["USDT", token_symbol],
                        &[swap_to_token_price, swap_to_usdt_price],
                        profit,
                        price_history.clone(),
                    );
                }
            }
        }

        Ok(opportunities)
    }
}
