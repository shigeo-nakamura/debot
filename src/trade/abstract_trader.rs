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
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{
    dex::{dex::TokenPair, Dex},
    token::Token,
};

use super::DBHandler;

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

#[derive(Debug, Clone, PartialEq)]
pub enum ReasonForSell {
    Liquidated,
    Expired,
    Others,
}

#[derive(Debug, Clone)]
pub struct TradeOpportunity {
    pub trader_name: String,
    pub dex_index: Vec<usize>,
    pub token_index: Vec<usize>,
    pub amounts: Vec<f64>,
    pub operation: Operation,
    pub reason_for_sell: Option<ReasonForSell>,
    pub predicted_profit: Option<f64>,
    pub currect_price: Option<f64>,
    pub predicted_price: Option<f64>,
    pub atr: Option<f64>,
}

impl TradeOpportunity {
    #[allow(dead_code)]
    pub fn print_info(&self, dexes: &[Box<dyn Dex>], tokens: &[Box<dyn Token>]) {
        let num_paths = self.dex_index.len();
        if let Some(profit) = self.predicted_profit {
            if profit > 0.0 {
                log::debug!("{} profit: {}", self.trader_name, profit);
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TraderState {
    Active,
    Liquidated,
    Paused,
}

pub struct BaseTrader {
    name: String,
    state: TraderState,
    leverage: f64,
    initial_amount: f64,
    allowance_factor: f64,
    tokens: Arc<Vec<Box<dyn Token>>>,
    base_token: Arc<Box<dyn Token>>,
    dexes: Arc<Vec<Box<dyn Dex>>>,
    dry_run: bool,
    gas: f64,
    prev_balance: Option<f64>,
    db_handler: Arc<Mutex<DBHandler>>,
    save_prices: bool,
}

impl BaseTrader {
    pub fn new(
        name: String,
        state: TraderState,
        leverage: f64,
        initial_amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        dry_run: bool,
        gas: f64,
        db_handler: Arc<Mutex<DBHandler>>,
        prev_balance: Option<f64>,
        save_prices: bool,
    ) -> Self {
        Self {
            name,
            state,
            leverage,
            initial_amount,
            allowance_factor,
            tokens,
            base_token,
            dexes,
            dry_run,
            gas,
            prev_balance,
            db_handler,
            save_prices,
        }
    }

    pub async fn get_amount_of_token(
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

                if self.dry_run {
                    continue;
                }

                let token_decimals = token.decimals().unwrap();
                let converted_amount = U256::from_dec_str(&format!(
                    "{:.0}",
                    min_managed_amount * self.allowance_factor * 10f64.powi(token_decimals as i32)
                ))?;

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
                log::trace!("{:?}", e);
                log::debug!("No price: {}-{}@{}", token_a_name, token_b_name, dex_name);
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

    pub async fn calculate_and_log_balance(
        &mut self,
        chain_name: &str,
        wallet_address: &Address,
    ) -> Option<f64> {
        let mut total_amount_in_base_token = 0.0;

        for token in self.tokens().iter() {
            if let Ok(amount) = Self::get_amount_of_token(*wallet_address, token).await {
                let dex_arc = Arc::new(self.dexes[0].clone());
                let token_arc = Arc::new(token.clone());
                let base_token_arc = Arc::new(self.base_token.clone());

                log::debug!("{}: {:6.6}", token.symbol_name(), amount);

                if token.symbol_name() == self.base_token.symbol_name() {
                    total_amount_in_base_token += amount;
                    continue;
                }

                if amount == 0.0 {
                    continue;
                }

                if let Some((_token_a_name, _token_b_name, _dex_name, price)) =
                    Self::fetch_token_prices(&dex_arc, &base_token_arc, &token_arc, amount, false)
                        .await
                {
                    total_amount_in_base_token += amount * price;
                }
            }
        }

        log::info!("log_current_balance: {:6.6}", total_amount_in_base_token);

        self.db_handler
            .lock()
            .await
            .log_current_balance(chain_name, total_amount_in_base_token, self.prev_balance)
            .await;

        Some(total_amount_in_base_token)
    }

    pub fn state(&self) -> TraderState {
        self.state.clone()
    }

    pub async fn set_state(&mut self, state: TraderState) {
        self.state = state;

        self.db_handler
            .lock()
            .await
            .log_trader_state(&self.name, self.state.clone())
            .await;
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

    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    #[allow(dead_code)]
    pub fn gas(&self) -> f64 {
        self.gas
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn db_handler(&self) -> &Arc<Mutex<DBHandler>> {
        &self.db_handler
    }

    pub fn save_prices(&self) -> bool {
        self.save_prices
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

    fn state(&self) -> TraderState;
    async fn set_state(&mut self, state: TraderState);
    fn leverage(&self) -> f64;
    fn initial_amount(&self) -> f64;
    fn tokens(&self) -> Arc<Vec<Box<dyn Token>>>;
    fn base_token(&self) -> Arc<Box<dyn Token>>;
    fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>>;
    fn name(&self) -> &str;
    fn db_handler(&self) -> &Arc<Mutex<DBHandler>>;

    async fn init(
        &mut self,
        owner: Address,
        min_managed_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>>;

    async fn transfer_token(
        &self,
        recipient: Address,
        token: &Box<dyn Token>,
        amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn calculate_and_log_balance(
        &mut self,
        chain_name: &str,
        wallet_address: &Address,
    ) -> Option<f64>;
}
