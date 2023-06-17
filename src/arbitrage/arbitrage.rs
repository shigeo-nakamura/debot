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
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::task::JoinHandle;

use crate::{
    dex::{dex::TokenPair, Dex},
    token::Token,
};

pub fn find_index<T, F>(list: &[T], predicate: F) -> Option<usize>
where
    F: Fn(&T) -> bool,
{
    list.iter().position(predicate)
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub dex_index: Vec<usize>,
    pub token_index: Vec<usize>,
    pub amounts: Vec<f64>,
    pub predicted_profit: Option<f64>,
    pub currect_price: Option<f64>,
    pub predicted_price: Option<f64>,
    pub gas: f64,
}

impl ArbitrageOpportunity {
    pub fn print_info(&self, dexes: &[Box<dyn Dex>], tokens: &[Box<dyn Token>]) {
        let num_paths = self.dex_index.len();
        if let Some(profit) = self.predicted_profit {
            if profit > 0.0 {
                log::info!("Profit: {}", profit);
                for i in 0..num_paths {
                    let dex = &dexes[self.dex_index[i]];
                    let token = &tokens[self.token_index[i]];
                    log::info!(
                        "  DEX: {}, Token: {}, Amount: {}",
                        dex.name(),
                        token.symbol_name(),
                        self.amounts[i]
                    );
                }
            } else {
                log::debug!("Loss: {} - {} = {}", profit + self.gas, self.gas, profit);
                for i in 0..num_paths {
                    let dex = &dexes[self.dex_index[i]];
                    let token = &tokens[self.token_index[i]];
                    log::debug!(
                        "  DEX: {}, Token: {}, Amount: {}",
                        dex.name(),
                        token.symbol_name(),
                        self.amounts[i]
                    );
                }
            }
        }
    }
}

pub struct BaseArbitrage {
    leverage: f64,
    initial_amount: f64,
    allowance_factor: f64,
    tokens: Arc<Vec<Box<dyn Token>>>,
    base_token: Arc<Box<dyn Token>>,
    dexes: Arc<Vec<Box<dyn Dex>>>,
    skip_write: bool,
    gas: f64,
}

impl BaseArbitrage {
    pub fn new(
        leverage: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
        gas: f64,
    ) -> Self {
        Self {
            leverage,
            initial_amount: 0.0,
            allowance_factor,
            tokens,
            base_token,
            dexes,
            skip_write,
            gas,
        }
    }

    pub async fn init(
        &mut self,
        owner: Address,
        min_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // todo: check gas token

        for token in self.tokens.iter() {
            for dex in self.dexes.iter() {
                // Check the allowed amount
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
                    min_amount * self.allowance_factor * 10f64.powi(token_decimals as i32)
                ))?;

                if self.skip_write {
                    // just for testing
                    self.initial_amount = 1000.0;
                    return Ok(());
                }

                if allowance < converted_amount / 2 {
                    token.approve(spender, converted_amount).await?;
                    log::info!(
                        "Approved {} {} for dex {}",
                        min_amount * self.allowance_factor,
                        token.symbol_name(),
                        dex.name(),
                    );
                }

                if token.symbol_name() == self.base_token.symbol_name() {
                    let base_token_amount = self.base_token.balance_of(owner).await?;
                    let base_token_amount =
                        base_token_amount.as_u128() as f64 / 10f64.powi(token_decimals as i32);
                    if base_token_amount < min_amount {
                        panic!("Not enough amount of base token");
                    }
                    self.initial_amount = base_token_amount;
                }
            }
        }
        Ok(())
    }

    async fn fetch_token_prices(
        dex: &Arc<Box<dyn Dex>>,
        token_a: &Arc<Box<dyn Token>>,
        token_b: &Arc<Box<dyn Token>>,
        amount: f64,
        reverse: bool,
    ) -> Option<(String, String, String, f64)> {
        let dex_name = dex.name().to_owned();
        let token_a_name = token_a.symbol_name().to_owned();
        let token_b_name = token_b.symbol_name().to_owned();

        let token_pair = TokenPair::new(token_a.clone(), token_b.clone());

        match dex.get_token_price(&token_pair, amount, reverse).await {
            Ok(price) => Some((token_a_name, token_b_name, dex_name, price)),
            Err(e) => {
                log::debug!("{:?}", e);
                log::trace!("No price: {}-{}@{}", token_a_name, token_b_name, dex_name);
                None
            }
        }
    }

    // Function to get token pair prices with base token
    pub async fn get_token_pair_prices(
        &self,
        dex: &Box<dyn Dex>,
        base_token: &Box<dyn Token>,
        tokens: &Vec<Box<dyn Token>>,
        amount: f64,
    ) -> Vec<JoinHandle<Result<Option<(String, String, String, f64)>, Box<dyn Error + Send + Sync>>>>
    {
        let mut get_price_futures = Vec::new();

        for token in tokens.iter() {
            if token.symbol_name() == base_token.symbol_name() {
                continue;
            }

            // buy price
            let fut_base = tokio::spawn({
                let dex_arc = Arc::new(dex.clone());
                let token_arc = Arc::new(token.clone());
                let base_token_arc = Arc::new(base_token.clone());
                async move {
                    let price_result = Self::fetch_token_prices(
                        &dex_arc,
                        &base_token_arc,
                        &token_arc,
                        amount,
                        false,
                    )
                    .await;
                    Ok(price_result)
                }
            });
            get_price_futures.push(fut_base);

            // sell price
            let fut_base = tokio::spawn({
                let dex_arc = Arc::new(dex.clone());
                let token_arc = Arc::new(token.clone());
                let base_token_arc = Arc::new(base_token.clone());
                async move {
                    let price_result = Self::fetch_token_prices(
                        &dex_arc,
                        &token_arc,
                        &base_token_arc,
                        amount,
                        true,
                    )
                    .await;
                    Ok(price_result)
                }
            });
            get_price_futures.push(fut_base);
        }

        get_price_futures
    }

    pub fn leverage(&self) -> f64 {
        self.leverage
    }

    pub fn initial_amount(&self) -> f64 {
        self.initial_amount
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

    pub fn gas(&self) -> f64 {
        self.gas
    }
}

#[async_trait]
pub trait Arbitrage {
    async fn execute_transactions(
        &mut self,
        opportunities: &Vec<ArbitrageOpportunity>,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn init(
        &mut self,
        owner: Address,
        min_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>>;
}
