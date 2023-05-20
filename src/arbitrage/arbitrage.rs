// arbitrage.rs

use async_trait::async_trait;
use ethers::{prelude::LocalWallet, types::Address};
use ethers_middleware::{
    providers::{Http, Provider},
    NonceManagerMiddleware, SignerMiddleware,
};
use std::{
    error::Error,
    sync::{Arc, RwLock},
};

use crate::http::TransactionResult;

pub struct ArbitrageOpportunity {
    pub dex1_index: usize,
    pub dex2_index: usize,
    pub token_a_index: usize,
    pub token_b_index: usize,
    pub profit: f64,
    pub amount: f64,
}

#[async_trait]
pub trait Arbitrage {
    async fn find_opportunities(
        &self,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>>;

    async fn execute_transactions(
        &self,
        opportunity: &ArbitrageOpportunity,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
        transactino_results: Arc<RwLock<Vec<TransactionResult>>>,
        log_limit: usize,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn store_transaction_result(
        transaction_result: TransactionResult,
        transaction_results: Arc<RwLock<Vec<TransactionResult>>>,
        limit: usize,
    ) {
        let mut transaction_results_guard = transaction_results.write().unwrap();
        transaction_results_guard.push(transaction_result);

        // If there are too many transaction results, remove the oldest ones until the limit is satisfied
        while transaction_results_guard.len() > limit {
            transaction_results_guard.remove(0);
        }
    }
}
