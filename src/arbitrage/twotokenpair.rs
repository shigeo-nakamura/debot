// twotokenpair.rs

use super::{Arbitrage, ArbitrageOpportunity};
use crate::dex::dex::TokenPair;
use crate::dex::Dex;
use crate::http::TransactionResult;
use crate::token::Token;
use anyhow::Context;
use async_trait::async_trait;
use ethers::prelude::LocalWallet;
use ethers::types::Address;
use ethers::types::U256;
use ethers_middleware::providers::Http;
use ethers_middleware::providers::Provider;
use ethers_middleware::NonceManagerMiddleware;
use ethers_middleware::SignerMiddleware;
use futures::future::join_all;
use std::error::Error;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::SystemTime;
use tokio::task::JoinHandle;

pub struct TwoTokenPairArbitrage {
    amount: f64,
    allowance_factor: f64,
    tokens: Arc<Vec<Box<dyn Token>>>,
    base_token: Arc<Box<dyn Token>>,
    dexes: Arc<Vec<Box<dyn Dex>>>,
}

impl<'a> TwoTokenPairArbitrage {
    pub fn new(
        amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
    ) -> Self {
        Self {
            amount,
            allowance_factor,
            tokens,
            base_token,
            dexes,
        }
    }

    pub async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>> {
        for token in self.tokens.iter() {
            for dex in self.dexes.iter() {
                let spender = dex.router_address();
                let allowance = token.allowance(owner, spender).await?;
                log::info!(
                    "Allowance for token {}: {} for dex {}",
                    token.symbol_name(),
                    allowance,
                    dex.name(),
                );

                let token_decimals = token.decimals().unwrap();
                let converted_amount = U256::from_dec_str(&format!(
                    "{:.0}",
                    self.amount * self.allowance_factor * 10f64.powi(token_decimals as i32)
                ))?;

                if allowance < converted_amount / 2 {
                    token.approve(spender, converted_amount).await?;
                    log::info!(
                        "Approved {} {} for dex {}",
                        self.amount,
                        token.symbol_name(),
                        dex.name(),
                    );
                }
            }
        }
        Ok(())
    }

    fn log_arbitrage_info(
        base_token: &Box<dyn Token>,
        dex1: &Box<dyn Dex>,
        dex2: &Box<dyn Dex>,
        token: &Box<dyn Token>,
        amount: f64,
        token_amount: f64,
        final_amount: f64,
        swap_to_token_price: f64,
        swap_to_base_token_price: f64,
        profit: f64,
        has_opportunity: bool,
    ) {
        let opportunity_string = if has_opportunity {
            "Arbitrage opportunity"
        } else {
            "No arbitrage opportunity"
        };

        let profit_string = if has_opportunity { "Profit" } else { "Loss" };

        log::info!(
            "{} [{} and {}] for ({} - {}). {}: {} {}",
            opportunity_string,
            dex1.name(),
            dex2.name(),
            base_token.symbol_name(),
            token.symbol_name(),
            profit_string,
            profit,
            base_token.symbol_name()
        );

        log::debug!(
            "{}({}) -> {}({}) -> {}({}), {}: {} {}/{}, {}: {} {}/{}",
            base_token.symbol_name(),
            amount,
            token.symbol_name(),
            token_amount,
            base_token.symbol_name(),
            final_amount,
            dex1.name(),
            swap_to_token_price,
            token.symbol_name(),
            base_token.symbol_name(),
            dex2.name(),
            swap_to_base_token_price,
            base_token.symbol_name(),
            token.symbol_name(),
        );
    }
}

#[async_trait]
impl Arbitrage for TwoTokenPairArbitrage {
    async fn find_opportunities(
        &self,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut tasks: Vec<JoinHandle<Result<Vec<ArbitrageOpportunity>, anyhow::Error>>> = vec![];

        for token in self.tokens.iter().cloned() {
            if token.symbol_name() == self.base_token.symbol_name() {
                continue;
            }

            let base_token = self.base_token.clone();
            let amount = self.amount;
            let dexes = self.dexes.clone();
            let tokens_cloned = self.tokens.clone();

            let task = tokio::spawn(async move {
                let mut opps: Vec<ArbitrageOpportunity> = vec![];

                let dexes_cloned = dexes.iter().map(|dex| dex.clone()).collect::<Vec<_>>();

                for (dex1_index, dex1) in dexes_cloned.iter().enumerate() {
                    for (dex2_index, dex2) in dexes_cloned.iter().enumerate() {
                        if dex1_index == dex2_index {
                            continue;
                        }

                        let swap_to_token_price = dex1
                            .get_token_price(
                                &TokenPair::new((*base_token).as_ref(), token.as_ref()),
                                amount,
                            )
                            .await
                            .map_err(|e| anyhow::anyhow!(e))
                            .context(format!(
                                "Error getting token price for {}/{} from dex1",
                                token.symbol_name(),
                                base_token.symbol_name()
                            ))?;
                        let token_b_amount = amount * swap_to_token_price;
                        let swap_to_usdt_price = dex2
                            .get_token_price(
                                &TokenPair::new(token.as_ref(), (*base_token).as_ref()),
                                token_b_amount,
                            )
                            .await
                            .map_err(|e| anyhow::anyhow!(e))
                            .context(format!(
                                "Error getting token price for {}/{} from dex2",
                                base_token.symbol_name(),
                                token.symbol_name()
                            ))?;
                        let final_amount = token_b_amount * swap_to_usdt_price;

                        let profit = final_amount - amount;
                        if profit > 0.0 {
                            Self::log_arbitrage_info(
                                &base_token,
                                dex1,
                                dex2,
                                &token,
                                amount,
                                token_b_amount,
                                final_amount,
                                swap_to_token_price,
                                swap_to_usdt_price,
                                profit,
                                true,
                            );

                            opps.push(ArbitrageOpportunity {
                                dex1_index,
                                dex2_index,
                                token_a_index: tokens_cloned
                                    .iter()
                                    .position(|t| t.symbol_name() == base_token.symbol_name())
                                    .unwrap(),
                                token_b_index: tokens_cloned
                                    .iter()
                                    .position(|t| t.symbol_name() == token.symbol_name())
                                    .unwrap(),
                                profit,
                                amount,
                            });
                        } else {
                            Self::log_arbitrage_info(
                                &base_token,
                                dex1,
                                dex2,
                                &token,
                                amount,
                                token_b_amount,
                                final_amount,
                                swap_to_token_price,
                                swap_to_usdt_price,
                                profit,
                                false,
                            );
                        }
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
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
        transaction_results: Arc<RwLock<Vec<TransactionResult>>>,
        log_limit: usize,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dex1 = &self.dexes[opportunity.dex1_index];
        let dex2 = &self.dexes[opportunity.dex2_index];

        let token_a = &self.tokens[opportunity.token_a_index];
        let token_b = &self.tokens[opportunity.token_b_index];

        log::info!(
            "Starting arbitrage: {} to {} via {} and {}",
            token_a.symbol_name(),
            token_b.symbol_name(),
            dex1.name(),
            dex2.name(),
        );

        let amount_a_to_b = dex1
            .swap_token(
                &TokenPair::new(token_a.as_ref(), token_b.as_ref()),
                opportunity.amount,
                wallet_and_provider.clone(),
                address,
                deadline_secs,
            )
            .await
            .map_err(|err| {
                format!(
                    "Failed to swap from {} to {} on dex {}: {}",
                    token_a.symbol_name(),
                    token_b.symbol_name(),
                    dex1.name(),
                    err
                )
            })?;

        log::debug!(
            "Swapped {} {} for {} {} on {}",
            opportunity.amount,
            token_a.symbol_name(),
            amount_a_to_b,
            token_b.symbol_name(),
            dex1.name(),
        );

        let amount_b_to_a = dex2
            .swap_token(
                &TokenPair::new(token_b.as_ref(), token_a.as_ref()),
                amount_a_to_b,
                wallet_and_provider.clone(),
                address,
                deadline_secs,
            )
            .await
            .map_err(|err| {
                format!(
                    "Failed to swap from {} to {} on dex {}: {}",
                    token_b.symbol_name(),
                    token_a.symbol_name(),
                    dex2.name(),
                    err
                )
            })?;
        log::debug!(
            "Swapped {} {} for {} {} on {}",
            amount_a_to_b,
            token_b.symbol_name(),
            amount_b_to_a,
            token_a.symbol_name(),
            dex2.name(),
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

        let transaction_result = TransactionResult {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            dex_names: vec![dex1.name().to_string(), dex2.name().to_string()],
            token_symbols: vec![
                token_a.symbol_name().to_string(),
                token_b.symbol_name().to_string(),
            ],
            amounts: vec![amount_a_to_b, amount_b_to_a],
            profit: amount_b_to_a - opportunity.amount,
        };

        Self::store_transaction_result(transaction_result, transaction_results, log_limit);

        Ok(())
    }
}
