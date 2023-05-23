// twotokenpair.rs

use super::arbitrage::BaseArbitrage;
use super::{find_index, Arbitrage, ArbitrageOpportunity};
use crate::dex::dex::TokenPair;
use crate::dex::Dex;
use crate::http::TransactionResult;
use crate::token::Token;
use anyhow::Context;
use async_trait::async_trait;
use ethers::prelude::LocalWallet;
use ethers::types::Address;
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
    base_arbitrage: BaseArbitrage,
}

impl<'a> TwoTokenPairArbitrage {
    pub fn new(
        amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
    ) -> Self {
        Self {
            base_arbitrage: BaseArbitrage::new(
                amount,
                allowance_factor,
                tokens,
                base_token,
                dexes,
                skip_write,
            ),
        }
    }

    pub fn amount(&self) -> f64 {
        self.base_arbitrage.amount()
    }

    pub fn tokens(&self) -> Arc<Vec<Box<dyn Token>>> {
        self.base_arbitrage.tokens()
    }

    pub fn base_token(&self) -> Arc<Box<dyn Token>> {
        self.base_arbitrage.base_token()
    }

    pub fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>> {
        self.base_arbitrage.dexes()
    }

    pub fn skip_write(&self) -> bool {
        self.base_arbitrage.skip_write()
    }
}

#[async_trait]
impl Arbitrage for TwoTokenPairArbitrage {
    async fn find_opportunities(
        &self,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut tasks: Vec<JoinHandle<Result<Vec<ArbitrageOpportunity>, anyhow::Error>>> = vec![];

        for token in self.tokens().iter().cloned() {
            if token.symbol_name() == self.base_token().symbol_name() {
                continue;
            }

            let base_token = self.base_token().clone();
            let amount = self.amount();
            let dexes = self.dexes().clone();
            let tokens_cloned = self.tokens().clone();

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
                                &TokenPair::new(base_token.clone(), Arc::new(token.clone())),
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
                                &TokenPair::new(Arc::new(token.clone()), Arc::clone(&base_token)),
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
                        let has_opportunity = profit > 0.0;

                        TwoTokenPairArbitrage::log_arbitrage_info(
                            dex1,
                            dex2,
                            None,
                            &base_token,
                            &token,
                            None,
                            amount,
                            token_b_amount,
                            final_amount,
                            None,
                            swap_to_token_price,
                            swap_to_usdt_price,
                            None,
                            profit,
                            has_opportunity,
                        );

                        if has_opportunity {
                            opps.push(ArbitrageOpportunity {
                                dex1_index,
                                dex2_index,
                                dex3_index: None,
                                token_a_index: tokens_cloned
                                    .iter()
                                    .position(|t| t.symbol_name() == base_token.symbol_name())
                                    .unwrap(),
                                token_b_index: tokens_cloned
                                    .iter()
                                    .position(|t| t.symbol_name() == token.symbol_name())
                                    .unwrap(),
                                token_c_index: None,
                                profit,
                                amount,
                            });
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
        if self.skip_write() {
            return Ok(());
        }

        let dex1 = &self.dexes()[opportunity.dex1_index];
        let dex2 = &self.dexes()[opportunity.dex2_index];

        let token_a = &self.tokens()[opportunity.token_a_index];
        let token_b = &self.tokens()[opportunity.token_b_index];

        log::info!(
            "Starting arbitrage: {} to {} via {} and {}",
            token_a.symbol_name(),
            token_b.symbol_name(),
            dex1.name(),
            dex2.name(),
        );

        let amount_a_to_b = dex1
            .swap_token(
                &TokenPair::new(Arc::new(token_a.clone()), Arc::new(token_b.clone())),
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
                &TokenPair::new(Arc::new(token_b.clone()), Arc::new(token_a.clone())),
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

        let gain = amount_b_to_a - opportunity.amount;
        if gain < 0.0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Arbitrage transaction resulted in a net loss: {} {}",
                    gain,
                    token_a.symbol_name()
                ),
            )));
        }

        log::info!(
            "Arbitrage complete. Net gain: {} {}",
            gain,
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

    async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_arbitrage.init(owner).await
    }
}
