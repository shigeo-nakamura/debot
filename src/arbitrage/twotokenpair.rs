// twotokenpair.rs

use super::{Arbitrage, ArbitrageOpportunity};
use crate::dex::Dex;
use crate::http::PriceData;
use crate::token::Token;
use anyhow::Context;
use async_trait::async_trait;
use ethers::prelude::LocalWallet;
use ethers::types::Address;
use futures::future::join_all;
use std::error::Error;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::task::JoinHandle;

pub struct TwoTokenPairArbitrage {
    amount: f64,
    tokens: Vec<Box<dyn Token>>,
    base_token: Box<dyn Token>,
    dexes: Arc<Vec<Box<dyn Dex>>>,
}

impl<'a> TwoTokenPairArbitrage {
    pub fn new(
        amount: f64,
        tokens: Vec<Box<dyn Token>>,
        base_token: Box<dyn Token>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
    ) -> Self {
        Self {
            amount,
            tokens,
            base_token,
            dexes,
        }
    }

    pub async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>> {
        for token in &self.tokens {
            for dex in self.dexes.iter() {
                let spender = dex.get_router_address();
                let allowance = token.allowance(owner, spender).await?;
                log::info!(
                    "Allowance for token {}: {} for dex {}",
                    token.symbol_name(),
                    allowance,
                    dex.get_name(),
                );
            }
        }
        Ok(())
    }

    fn log_arbitrage_opportunity(
        dex1: &Box<dyn Dex>,
        dex2: &Box<dyn Dex>,
        token: &Box<dyn Token>,
        amount: f64,
        token_b_amount: f64,
        swap_to_token_price: f64,
        swap_to_usdt_price: f64,
        profit: f64,
    ) {
        log::info!(
            "Arbitrage opportunity [{} and {}] for (USDT - {}). Profit: {} USDT",
            dex1.get_name(),
            dex2.get_name(),
            token.symbol_name(),
            profit
        );
        log::debug!(
            "Dex 1: {}, USDT --> {}, Input Amount: {}, Output Amount: {}, price: {}",
            dex1.get_name(),
            token.symbol_name(),
            amount,
            token_b_amount,
            swap_to_token_price,
        );
        log::debug!(
            "Dex 2 {}, {} --> USDT, Input Amount: {}, Output Amount: {}, price: {}",
            dex2.get_name(),
            token.symbol_name(),
            token_b_amount,
            token_b_amount * swap_to_usdt_price,
            swap_to_usdt_price
        );
    }

    fn log_no_arbitrage_opportunity(
        dex1: &Box<dyn Dex>,
        dex2: &Box<dyn Dex>,
        token: &Box<dyn Token>,
        amount: f64,
        token_b_amount: f64,
        swap_to_token_price: f64,
        swap_to_usdt_price: f64,
        profit: f64,
    ) {
        log::info!(
            "No arbitrage opportunity [{} and {}] for (USDT - {}). Loss: {} USDT",
            dex1.get_name(),
            dex2.get_name(),
            token.symbol_name(),
            profit
        );
        log::debug!(
            "Dex 1: {}, USDT --> {}, Input Amount: {}, Output Amount: {}, price: {}",
            dex1.get_name(),
            token.symbol_name(),
            amount,
            token_b_amount,
            swap_to_token_price,
        );
        log::debug!(
            "Dex 2 {}, {} --> USDT, Input Amount: {}, Output Amount: {}, price: {}",
            dex2.get_name(),
            token.symbol_name(),
            token_b_amount,
            token_b_amount * swap_to_usdt_price,
            swap_to_usdt_price
        );
    }
}

#[async_trait]
impl Arbitrage for TwoTokenPairArbitrage {
    async fn find_opportunities(
        &self,
        price_history: Arc<RwLock<Vec<PriceData>>>,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut tasks: Vec<JoinHandle<Result<Vec<ArbitrageOpportunity>, anyhow::Error>>> = vec![];

        for token in self.tokens.iter().cloned() {
            if token.symbol_name() == self.base_token.symbol_name() {
                continue;
            }

            let price_history = price_history.clone();
            let base_token = self.base_token.clone();
            let amount = self.amount;
            let dexes = self.dexes.clone();
            let tokens_cloned = Arc::new(self.tokens.clone());

            let task = tokio::spawn(async move {
                let mut opps: Vec<ArbitrageOpportunity> = vec![];

                let dexes_cloned = dexes.iter().map(|dex| dex.clone()).collect::<Vec<_>>();

                for (dex1_index, dex1) in dexes_cloned.iter().enumerate() {
                    for (dex2_index, dex2) in dexes_cloned.iter().enumerate() {
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
                            TwoTokenPairArbitrage::log_arbitrage_opportunity(
                                dex1,
                                dex2,
                                &token,
                                amount,
                                token_b_amount,
                                swap_to_token_price,
                                swap_to_usdt_price,
                                profit,
                            );

                            opps.push(ArbitrageOpportunity {
                                dex1_index,
                                dex2_index,
                                token_a_index: 0,
                                token_b_index: tokens_cloned
                                    .iter()
                                    .position(|t| t.symbol_name() == token.symbol_name())
                                    .unwrap(),
                                profit,
                                amount,
                            });
                        } else {
                            TwoTokenPairArbitrage::log_no_arbitrage_opportunity(
                                dex1,
                                dex2,
                                &token,
                                amount,
                                token_b_amount,
                                swap_to_token_price,
                                swap_to_usdt_price,
                                profit,
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

    async fn execute_transactions(
        &self,
        opportunity: &ArbitrageOpportunity,
        signer: &LocalWallet,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dex1 = &self.dexes[opportunity.dex1_index];
        let dex2 = &self.dexes[opportunity.dex2_index];

        let token_a = &self.tokens[opportunity.token_a_index];
        let token_b = &self.tokens[opportunity.token_b_index];

        log::info!(
            "Starting arbitrage: {} to {} via {} and {}",
            token_a.symbol_name(),
            token_b.symbol_name(),
            dex1.get_name(),
            dex2.get_name(),
        );

        let amount_a_to_b = dex1
            .swap_token(
                token_a.as_ref(),
                token_b.as_ref(),
                opportunity.amount,
                signer,
            )
            .await
            .map_err(|err| {
                format!(
                    "Failed to swap from {} to {} on dex {}: {}",
                    token_a.symbol_name(),
                    token_b.symbol_name(),
                    dex1.get_name(),
                    err
                )
            })?;

        log::info!(
            "Swapped {} {} for {} {} on {}",
            opportunity.amount,
            token_a.symbol_name(),
            amount_a_to_b,
            token_b.symbol_name(),
            dex1.get_name(),
        );

        let amount_b_to_a = dex2
            .swap_token(token_b.as_ref(), token_a.as_ref(), amount_a_to_b, signer)
            .await
            .map_err(|err| {
                format!(
                    "Failed to swap from {} to {} on dex {}: {}",
                    token_b.symbol_name(),
                    token_a.symbol_name(),
                    dex2.get_name(),
                    err
                )
            })?;
        log::info!(
            "Swapped {} {} for {} {} on {}",
            amount_a_to_b,
            token_b.symbol_name(),
            amount_b_to_a,
            token_a.symbol_name(),
            dex2.get_name(),
        );

        if amount_b_to_a < opportunity.amount {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Arbitrage transaction resulted in a net loss",
            )));
        }

        log::info!(
            "Arbitrage complete. Net gain: {} {}",
            amount_b_to_a - opportunity.amount,
            token_a.symbol_name(),
        );

        Ok(())
    }
}
