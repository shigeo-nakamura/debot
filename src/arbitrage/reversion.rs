use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;

use crate::arbitrage::find_index;
use crate::arbitrage::Arbitrage;
use crate::dex::dex::TokenPair;
use crate::dex::Dex;
use crate::token::Token;

use async_trait::async_trait;
use ethers::prelude::{Provider, SignerMiddleware};
use ethers::providers::Http;
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::NonceManagerMiddleware;

use super::arbitrage::BaseArbitrage;
use super::{ArbitrageOpportunity, PriceHistory};

pub struct OpenPosition {
    averate_price: f64,
    amount: f64,
    open_time: usize,
}

pub struct ReversionArbitrage {
    base_arbitrage: BaseArbitrage,
    short_trade_period: usize,
    long_trade_period: usize,
    loss_limit_ratio: f64,
    profit_limit_ratio: f64,
    max_position_amount: f64,
    max_hold_period: usize,
    match_multiplier: f64,
    mismatch_multiplier: f64,
    open_positions: HashMap<String, OpenPosition>,
}

impl ReversionArbitrage {
    pub fn new(
        amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
        gas: f64,
        short_trade_period: usize,
        long_trade_period: usize,
        loss_limit_ratio: f64,
        profit_limit_ratio: f64,
        max_position_amount: f64,
        max_hold_period: usize,
        match_multiplier: f64,
        mismatch_multiplier: f64,
    ) -> Self {
        Self {
            base_arbitrage: BaseArbitrage::new(
                amount,
                allowance_factor,
                tokens,
                base_token,
                dexes,
                skip_write,
                gas,
            ),
            short_trade_period,
            long_trade_period,
            loss_limit_ratio,
            profit_limit_ratio,
            max_position_amount,
            max_hold_period,
            match_multiplier,
            mismatch_multiplier,
            open_positions: HashMap::new(),
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

    fn can_open_position(&self) -> bool {
        let mut amount = 0.0;
        for (_token_name, position) in &self.open_positions {
            amount += position.amount;
        }
        amount < self.max_position_amount
    }

    async fn get_current_prices(
        &self,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        // Get the prices of token pairs
        let token_pair_prices: HashMap<(String, String, String), f64> =
            self.get_token_pair_prices().await?;

        for ((token_a, token_b, dex), price) in &token_pair_prices {
            log::debug!(
                "Token pair price: {}-{}@{}: {}",
                token_a,
                token_b,
                dex,
                price
            );

            // Update the price history and predict next prices
            if token_a == self.base_token().symbol_name() {
                let history = histories.entry(token_b.clone()).or_insert_with(|| {
                    PriceHistory::new(self.short_trade_period, self.long_trade_period)
                });
                history.add_price(chrono::Utc::now().timestamp(), *price);
            }
        }

        Ok(token_pair_prices)
    }

    fn find_buy_opportunities(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
        histories: &HashMap<String, PriceHistory>,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<ArbitrageOpportunity> = vec![];
        let next_timestamp = chrono::Utc::now().timestamp() + self.short_trade_period as i64;

        for ((token_a_name, token_b_name, dex_name), price) in current_prices {
            if token_a_name == self.base_token().symbol_name() {
                if let Some(history) = histories.get(token_b_name) {
                    let predicted_price_ema = history.predict_next_price_ema();
                    let predicted_price_regression =
                        history.predict_next_price_regression(next_timestamp);

                    log::debug!(
                        "Current price for {}: {}, EMA Prediction: {}, Regression Prediction: {}",
                        token_b_name,
                        price,
                        predicted_price_ema,
                        predicted_price_regression
                    );

                    let predicted_price = match predicted_price_ema > predicted_price_regression {
                        true => predicted_price_ema,
                        false => predicted_price_regression,
                    };

                    let amount =
                        match predicted_price_ema > *price && predicted_price_regression > *price {
                            true => self.amount() * self.match_multiplier,
                            false => self.amount() * self.mismatch_multiplier,
                        };

                    if predicted_price > *price {
                        let token_a_index =
                            find_index(&self.tokens(), |token| token.symbol_name() == token_a_name)
                                .ok_or("Token not found")?;
                        let token_b_index =
                            find_index(&self.tokens(), |token| token.symbol_name() == token_b_name)
                                .ok_or("Token not found")?;
                        let dex_index = find_index(&self.dexes(), |dex| dex.name() == dex_name)
                            .ok_or("Dex not found")?;
                        let profit = (predicted_price - price) * self.amount();

                        if profit > self.profit_limit_ratio {
                            opportunities.push(ArbitrageOpportunity {
                                dex_index: vec![dex_index],
                                token_index: vec![token_a_index, token_b_index],
                                amounts: vec![amount],
                                profit,
                                gas: self.base_arbitrage.gas(),
                            });
                        }
                    }
                }
            }
        }

        Ok(opportunities)
    }

    fn find_sell_opportunities(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<ArbitrageOpportunity> = vec![];

        let current_time = chrono::Utc::now().timestamp() as usize;

        for ((token_a_name, token_b_name, dex_name), current_price) in current_prices {
            if token_b_name == self.base_token().symbol_name() {
                if let Some(position) = self.open_positions.get(token_a_name) {
                    let holding_period = current_time - position.open_time;
                    let potential_profit =
                        (current_price - position.averate_price) * position.amount;
                    let potential_loss = (position.averate_price - current_price) * position.amount;

                    if holding_period > self.max_hold_period
                        || potential_profit > position.amount * self.profit_limit_ratio
                        || potential_loss < position.amount * self.loss_limit_ratio
                    {
                        let token_a_index =
                            find_index(&self.tokens(), |token| token.symbol_name() == token_a_name)
                                .ok_or("Token not found")?;
                        let token_b_index =
                            find_index(&self.tokens(), |token| token.symbol_name() == token_b_name)
                                .ok_or("Token not found")?;
                        let dex_index = find_index(&self.dexes(), |dex| dex.name() == dex_name)
                            .ok_or("Dex not found")?;

                        opportunities.push(ArbitrageOpportunity {
                            dex_index: vec![dex_index],
                            token_index: vec![token_b_index, token_a_index],
                            amounts: vec![position.amount],
                            profit: potential_profit.abs(),
                            gas: self.base_arbitrage.gas(),
                        });
                    }
                }
            }
        }

        Ok(opportunities)
    }

    pub async fn find_opportunities(
        &self,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let current_prices: HashMap<(String, String, String), f64> =
            self.get_current_prices(histories).await?;

        let mut results: Vec<ArbitrageOpportunity> = vec![];

        let mut result_for_open = self.find_buy_opportunities(&current_prices, &histories)?;
        results.append(&mut result_for_open);

        let mut result_for_close = self.find_sell_opportunities(&current_prices)?;
        results.append(&mut result_for_close);

        Ok(results)
    }
}

#[async_trait]
impl Arbitrage for ReversionArbitrage {
    async fn execute_transactions(
        &mut self,
        opportunities: &Vec<ArbitrageOpportunity>,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
        log_limit: usize,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for opportunity in opportunities {
            let token_a = &self.tokens()[opportunity.token_index[0]];
            let token_b = &self.tokens()[opportunity.token_index[1]];

            let token_pair = TokenPair::new(Arc::new(token_a.clone()), Arc::new(token_b.clone()));

            let amount = opportunity.amounts[0];
            let dex = &self.dexes()[opportunity.dex_index[0]];

            // execute swap operation
            if !self.base_arbitrage.skip_write() {
                let _ = dex
                    .swap_token(
                        &token_pair,
                        amount,
                        wallet_and_provider.clone(),
                        address,
                        deadline_secs,
                    )
                    .await?;
            }

            // update positions
            let token_a_name = token_a.symbol_name();
            let token_b_name = token_b.symbol_name();
            let average_price = opportunity.profit / amount; // This is a simplification. You may want to compute the average price differently.

            log::debug!(
                "Processing opportunity with {} - {}",
                token_a_name,
                token_b_name
            );

            if token_a_name == self.base_token().symbol_name() {
                // This is a buy operation
                if !self.can_open_position() {
                    log::debug!("Cannot open new position.");
                    continue;
                }

                if let Some(position) = self.open_positions.get_mut(token_b_name) {
                    // if there are already open positions for this token, update them
                    position.amount += amount;
                    position.averate_price = (position.averate_price * position.amount
                        + average_price * amount)
                        / (position.amount + amount); // update the average price
                    log::debug!(
                        "Updated open position for token: {}, amount: {}, average price: {}",
                        token_b_name,
                        position.amount,
                        position.averate_price
                    );
                } else {
                    // else, create a new position
                    let open_position = OpenPosition {
                        averate_price: average_price,
                        amount,
                        open_time: chrono::Utc::now().timestamp() as usize,
                    };
                    log::debug!(
                        "Created new open position for token: {}, amount: {}, average price: {}, open time: {}",
                        token_b_name,
                        open_position.amount,
                        open_position.averate_price,
                        open_position.open_time,
                    );
                    self.open_positions
                        .insert(token_b_name.to_owned(), open_position);
                }
            } else {
                // This was a sell operation
                if let Some(position) = self.open_positions.get_mut(token_a_name) {
                    position.amount -= amount; // reduce the amount of the open position

                    if position.amount <= 0.0 {
                        self.open_positions.remove(token_a_name); // If all of this token has been sold, remove it from the open positions
                        log::debug!(
                            "Sold all of token: {}. Removed from open positions.",
                            token_a_name
                        );
                    } else {
                        position.averate_price = (position.averate_price * position.amount
                            + average_price * amount)
                            / (position.amount + amount); // update the average price
                        log::debug!(
                            "Updated open position for token: {}, amount: {}, average price: {}",
                            token_a_name,
                            position.amount,
                            position.averate_price
                        );
                    }
                }
            }
        }
        Ok(())
    }

    async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_arbitrage.init(owner).await
    }

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        // Get the prices of all token pairs
        let mut get_price_futures = Vec::new();
        let base_token = &self.base_token();
        let dexes = &self.dexes();
        let tokens = &self.tokens();
        let amount = self.amount();

        // Get prices with base token / each token and each token / base token
        for dex in dexes.iter() {
            let mut dex_get_price_futures = self
                .base_arbitrage
                .get_token_pair_prices(dex, base_token, tokens, amount)
                .await;
            get_price_futures.append(&mut dex_get_price_futures);
        }

        // Wait for all token price futures to finish
        let prices_results: Vec<
            Result<
                Result<Option<(String, String, String, f64)>, Box<dyn Error + Send + Sync>>,
                tokio::task::JoinError,
            >,
        > = futures::future::join_all(get_price_futures).await;

        let mut token_pair_prices = HashMap::new();

        // Aggregate all the prices into a HashMap
        for price_result in prices_results.into_iter() {
            if let Ok(Ok(Some((token_1_symbol, token_2_symbol, dex_name, price)))) = price_result {
                token_pair_prices.insert((token_1_symbol, token_2_symbol, dex_name), price);
            }
        }

        Ok(token_pair_prices)
    }
}
