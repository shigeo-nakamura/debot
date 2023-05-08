// twotokenpair.rs

use super::{Arbitrage, ArbitrageOpportunity};
use crate::dex::Dex;
use crate::http::PriceData;
use crate::token::Token;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use futures::future::join_all;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::task::JoinHandle;

pub struct TwoTokenPairArbitrage {
    amount: f64,
    tokens: Vec<Box<dyn Token>>,
    base_token: Box<dyn Token>,
}

impl TwoTokenPairArbitrage {
    pub fn new(amount: f64, tokens: Vec<Box<dyn Token>>, base_token: Box<dyn Token>) -> Self {
        Self {
            amount,
            tokens,
            base_token,
        }
    }
}

#[async_trait]
impl Arbitrage for TwoTokenPairArbitrage {
    async fn find_opportunities(
        &self,
        dexes: &Arc<Vec<(String, Box<dyn Dex>)>>,
        price_history: Arc<RwLock<Vec<PriceData>>>,
    ) -> anyhow::Result<Vec<ArbitrageOpportunity>> {
        let mut tasks: Vec<JoinHandle<Result<Vec<ArbitrageOpportunity>, anyhow::Error>>> = vec![];

        for token in self.tokens.iter().cloned() {
            if token.symbol_name() == self.base_token.symbol_name() {
                continue;
            }

            let dexes = Arc::clone(dexes);
            let price_history = price_history.clone();
            let base_token = self.base_token.clone();
            let amount = self.amount;

            let task = tokio::spawn(async move {
                let mut opps: Vec<ArbitrageOpportunity> = vec![];

                let dexes_cloned = dexes
                    .iter()
                    .map(|(name, dex)| (name.clone(), dex.clone()))
                    .collect::<Vec<_>>();

                for (dex1_index, (_dex1_name, dex1)) in dexes_cloned.iter().enumerate() {
                    for (dex2_index, (_dex2_name, dex2)) in dexes_cloned.iter().enumerate() {
                        if dex1_index == dex2_index {
                            continue;
                        }

                        let swap_to_token_price = dex1
                            .get_token_price(&*base_token, token.as_ref(), amount)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))
                            .context("Error getting token price from dex1")?;
                        let token_b_amount = amount * swap_to_token_price;

                        log::debug!(
                        "Dex 1: {}, USDT --> {}, Input Amount: {}, Output Amount: {}, price: {}",
                        dex1.get_name(),
                        token.symbol_name(),
                        amount,
                        token_b_amount,
                        swap_to_token_price,
                    );

                        let swap_to_usdt_price = dex2
                            .get_token_price(token.as_ref(), &*base_token, token_b_amount)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))
                            .context("Error getting token price from dex2")?;
                        let final_usdt_amount = token_b_amount * swap_to_usdt_price;

                        log::debug!(
                            "Dex 2 {}, {} --> USDT, Input Amount: {}, Output Amount: {}, price: {}",
                            dex2.get_name(),
                            token.symbol_name(),
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
                            token.symbol_name(),
                            profit
                        );
                            opps.push(ArbitrageOpportunity {
                                dex1_index,
                                dex2_index,
                                token_a_index: 0,
                                token_b_index: dexes_cloned
                                    .iter()
                                    .position(|t| t.0 == token.symbol_name())
                                    .unwrap(),
                                profit,
                                amount,
                            });
                        } else {
                            log::info!(
                            "No arbitrage opportunity [{} and {}] for (USDT - {}). Loss: {} USDT",
                            dex1.get_name(),
                            dex2.get_name(),
                            token.symbol_name(),
                            profit
                        );
                        }

                        // Store price data in price_history
                        TwoTokenPairArbitrage::store_price_history(
                            &[dex1.get_name(), dex2.get_name()],
                            &["USDT", token.symbol_name()],
                            &[swap_to_token_price, swap_to_usdt_price],
                            profit,
                            price_history.clone(),
                        );
                    }
                }

                Ok(opps)
            });

            tasks.push(task);
        }

        let mut opportunities: Vec<ArbitrageOpportunity> = vec![];
        let results = join_all(tasks).await;
        for result in results {
            match result {
                Ok(Ok(opps)) => opportunities.extend(opps),
                Ok(Err(e)) => log::error!("Error finding arbitrage opportunities: {:?}", e),
                Err(e) => log::error!("Error in spawned task: {:?}", e),
            }
        }

        Ok(opportunities)
    }
}
