// arbitrage.rs

use async_trait::async_trait;
use ethers::{
    prelude::LocalWallet,
    types::{Address, U256},
};
use ethers_middleware::{
    providers::{Http, Provider},
    NonceManagerMiddleware, SignerMiddleware,
};
use std::{
    error::Error,
    sync::{Arc, RwLock},
};

use crate::{dex::Dex, http::TransactionResult, token::Token};

pub fn find_index<T, F>(list: &[T], predicate: F) -> Option<usize>
where
    F: Fn(&T) -> bool,
{
    list.iter().position(predicate)
}

pub struct ArbitrageOpportunity {
    pub dex1_index: usize,
    pub dex2_index: usize,
    pub dex3_index: Option<usize>,
    pub token_a_index: usize,
    pub token_b_index: usize,
    pub token_c_index: Option<usize>,
    pub profit: f64,
    pub amount: f64,
}

pub struct BaseArbitrage {
    amount: f64,
    allowance_factor: f64,
    tokens: Arc<Vec<Box<dyn Token>>>,
    base_token: Arc<Box<dyn Token>>,
    dexes: Arc<Vec<Box<dyn Dex>>>,
    skip_write: bool,
}

impl BaseArbitrage {
    pub fn new(
        amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
    ) -> Self {
        Self {
            amount,
            allowance_factor,
            tokens,
            base_token,
            dexes,
            skip_write,
        }
    }

    pub async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>> {
        for token in self.tokens.iter() {
            for dex in self.dexes.iter() {
                let spender = dex.router_address();
                let allowance = token.allowance(owner, spender).await?;
                log::debug!(
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

                if self.skip_write {
                    return Ok(());
                }

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

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn tokens(&self) -> Arc<Vec<Box<dyn Token>>> {
        Arc::clone(&self.tokens)
    }

    pub fn base_token(&self) -> Arc<Box<dyn Token>> {
        Arc::clone(&self.base_token)
    }

    pub fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>> {
        Arc::clone(&self.dexes)
    }

    pub fn skip_write(&self) -> bool {
        self.skip_write
    }
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
        transaction_results: Arc<RwLock<Vec<TransactionResult>>>,
        log_limit: usize,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>>;

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

    fn log_transaction_result(&self, transaction_result: &TransactionResult) {
        log::info!(
            "Transaction Result at timestamp: {}, DEXes: {:?}, \
            Tokens: {:?}, Amounts: {:?}, Profit: {}",
            transaction_result.timestamp,
            transaction_result.dex_names,
            transaction_result.token_symbols,
            transaction_result.amounts,
            transaction_result.profit,
        );
    }

    fn log_arbitrage_info(
        dex1: &Box<dyn Dex>,
        dex2: &Box<dyn Dex>,
        dex3: Option<&Box<dyn Dex>>,
        token_a: &Box<dyn Token>,
        token_b: &Box<dyn Token>,
        token_c: Option<&Box<dyn Token>>,
        amount: f64,
        amount_a: f64,
        amount_b: f64,
        amount_c: Option<f64>,
        dex1_price: f64,
        dex2_price: f64,
        dex3_price: Option<f64>,
        profit: f64,
        has_opportunity: bool,
    ) {
        let opportunity_string = if has_opportunity {
            "! FOUND opportunity"
        } else {
            "No opportunity"
        };

        let profit_string = if has_opportunity { "Profit" } else { "Loss" };

        if dex3.is_none() {
            log::debug!(
                "{}({}) -> {}({}) -> {}({}), {}:{} {}/{}, {}:{} {}/{}",
                token_a.symbol_name(),
                amount,
                token_b.symbol_name(),
                amount_a,
                token_a.symbol_name(),
                amount_b,
                dex1.name(),
                dex1_price,
                token_b.symbol_name(),
                token_a.symbol_name(),
                dex2.name(),
                dex2_price,
                token_a.symbol_name(),
                token_b.symbol_name()
            );

            log::info!(
                "{} [{},{}] for ({}-{}). {}: {} {}",
                opportunity_string,
                dex1.name(),
                dex2.name(),
                token_a.symbol_name(),
                token_b.symbol_name(),
                profit_string,
                profit,
                token_a.symbol_name()
            );
        } else {
            let token_c = token_c.unwrap();
            let amount_c = amount_c.unwrap();
            let dex3 = dex3.unwrap();
            let dex3_price = dex3_price.unwrap();
            log::debug!(
                "{}({}) -> {}({}) -> {}({}) -> {}({}), {}:{} {}/{}, {}:{} {}/{}, {}:{} {}/{}",
                token_a.symbol_name(),
                amount,
                token_b.symbol_name(),
                amount_a,
                token_c.symbol_name(),
                amount_b,
                token_a.symbol_name(),
                amount_c,
                dex1.name(),
                dex1_price,
                token_b.symbol_name(),
                token_a.symbol_name(),
                dex2.name(),
                dex2_price,
                token_c.symbol_name(),
                token_b.symbol_name(),
                dex3.name(),
                dex3_price,
                token_a.symbol_name(),
                token_c.symbol_name()
            );

            log::info!(
                "{} [{}, {},{}] for ({}-{}-{}). {}: {} {}",
                opportunity_string,
                dex1.name(),
                dex2.name(),
                dex3.name(),
                token_a.symbol_name(),
                token_b.symbol_name(),
                token_c.symbol_name(),
                profit_string,
                profit,
                token_a.symbol_name()
            );
        }
    }
}
