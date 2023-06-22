// abstract_trader.rs

use async_trait::async_trait;
use ethers::{
    prelude::LocalWallet,
    types::{Address, U256},
};
use ethers_middleware::{
    providers::{Http, Provider},
    NonceManagerMiddleware, SignerMiddleware,
};
use shared_mongodb::ClientHolder;
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
};
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

#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct TradeOpportunity {
    pub trader_name: String,
    pub dex_index: Vec<usize>,
    pub token_index: Vec<usize>,
    pub amounts: Vec<f64>,
    pub operation: Operation,
    pub predicted_profit: Option<f64>,
    pub currect_price: Option<f64>,
    pub predicted_price: Option<f64>,
}

impl TradeOpportunity {
    pub fn print_info(&self, dexes: &[Box<dyn Dex>], tokens: &[Box<dyn Token>]) {
        let num_paths = self.dex_index.len();
        if let Some(profit) = self.predicted_profit {
            if profit > 0.0 {
                log::info!("{} profit: {}", self.trader_name, profit);
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
                log::debug!("{} loss: {}", self.trader_name, profit);
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

pub struct BaseTrader {
    name: String,
    leverage: f64,
    initial_amount: f64,
    allowance_factor: f64,
    tokens: Arc<Vec<Box<dyn Token>>>,
    base_token: Arc<Box<dyn Token>>,
    dexes: Arc<Vec<Box<dyn Dex>>>,
    skip_write: bool,
    gas: f64,
    client_holder: Arc<Mutex<ClientHolder>>,
}

impl BaseTrader {
    pub fn new(
        name: String,
        leverage: f64,
        initial_amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
        gas: f64,
        client_holder: Arc<Mutex<ClientHolder>>,
    ) -> Self {
        Self {
            name,
            leverage,
            initial_amount,
            allowance_factor,
            tokens,
            base_token,
            dexes,
            skip_write,
            gas,
            client_holder,
        }
    }

    pub async fn get_amount_of_token(
        &self,
        owner: Address,
        token: &Box<dyn Token>,
    ) -> Result<f64, Box<dyn Error + Send + Sync>> {
        let token_decimals = token.decimals().unwrap();
        let token_amount = token.balance_of(owner).await?;
        let token_amount = token_amount.as_u128() as f64 / 10f64.powi(token_decimals as i32);
        Ok(token_amount)
    }

    pub async fn transfer_token(
        &self,
        recipient: Address,
        token: &Box<dyn Token>,
        amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let token_decimals = token.decimals().unwrap();
        let converted_amount = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(token_decimals as i32)
        ))?;
        token.transfer(recipient, converted_amount).await?;
        Ok(())
    }

    pub async fn init(
        &mut self,
        owner: Address,
        min_managed_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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
                    min_managed_amount * self.allowance_factor * 10f64.powi(token_decimals as i32)
                ))?;

                if token.symbol_name() == self.base_token.symbol_name() {
                    let base_token_amount = self.get_amount_of_token(owner, token).await?;
                    if base_token_amount < min_managed_amount {
                        if !self.skip_write {
                            panic!("Not enough amount of base token");
                        }
                    }
                    log::info!("basek_token_amount = {:6.3}", base_token_amount);
                    self.initial_amount = min_managed_amount;
                }

                if self.skip_write {
                    continue;
                }

                if allowance < converted_amount {
                    // Convert f64 to U256
                    let allowance_factor_u256 =
                        U256::from_dec_str(&(self.allowance_factor.to_string())).unwrap();
                    let allowed_amount: U256 = converted_amount * allowance_factor_u256;
                    token.approve(spender, allowed_amount).await?;
                    log::info!(
                        "Approved {} {} for dex {}",
                        allowed_amount,
                        token.symbol_name(),
                        dex.name(),
                    );
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn db_client(&self) -> &Arc<Mutex<ClientHolder>> {
        &self.client_holder
    }
}

#[async_trait]
pub trait AbstractTrader {
    async fn execute_transactions(
        &mut self,
        opportunities: &Vec<TradeOpportunity>,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn leverage(&self) -> f64;
    fn initial_amount(&self) -> f64;
    fn tokens(&self) -> Arc<Vec<Box<dyn Token>>>;
    fn base_token(&self) -> Arc<Box<dyn Token>>;
    fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>>;
    fn name(&self) -> &str;
    fn db_client(&self) -> &Arc<Mutex<ClientHolder>>;

    async fn init(
        &mut self,
        owner: Address,
        min_managed_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>>;

    async fn get_amount_of_token(
        &self,
        owner: Address,
        token: &Box<dyn Token>,
    ) -> Result<f64, Box<dyn Error + Send + Sync>>;

    async fn transfer_token(
        &self,
        recipient: Address,
        token: &Box<dyn Token>,
        amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
