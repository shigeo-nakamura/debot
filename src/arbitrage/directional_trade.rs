use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use crate::arbitrage::find_index;
use crate::arbitrage::Arbitrage;
use crate::arbitrage::TradingStrategy;
use crate::dex::dex::TokenPair;
use crate::dex::Dex;
use crate::token::Token;

use async_trait::async_trait;
use ethers::prelude::{Provider, SignerMiddleware};
use ethers::providers::Http;
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::NonceManagerMiddleware;
use serde::{Deserialize, Serialize};

use super::arbitrage::BaseArbitrage;
use super::{ArbitrageOpportunity, PriceHistory};

#[derive(Serialize, Deserialize)]
pub struct DirectionalTradeLog {
    open_time: i64,
    close_time: i64,
    event_type: String, // "take profit", "loss cut", ""hold period over
    token: String,      // token against the base token
    average_price: f64,
    position_amount: f64,
    realized_pnl: f64, // realized profit or loss
}

pub struct OpenPosition {
    open_time: usize,
    average_price: f64,
    amount: f64,
}

impl OpenPosition {
    pub fn new(average_price: f64, amount: f64) -> Self {
        Self {
            open_time: chrono::Utc::now().timestamp() as usize,
            average_price,
            amount,
        }
    }

    pub fn print_info(&self, token_name: &str, current_price: f64) -> () {
        let pnl = (current_price - self.average_price) * self.amount;
        log::debug!(
            "{} PNL = {:6.3}, current_price: {:6.3}, average_price: {:6.3}, amount: {:6.6}",
            token_name,
            pnl,
            current_price,
            self.average_price,
            self.amount
        );
    }
}

pub struct DirectionalTradeConfig {
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
    percentage_loss_threshold: f64,
    percentage_profit_threshold: f64,
    percentage_drop_threshold: f64,
    max_position_amount: f64,
    max_hold_period: usize,
    match_multiplier: f64,
    mismatch_multiplier: f64,
    log_limit: usize,
}

pub struct DirectionalTradeState {
    open_positions: HashMap<String, OpenPosition>,
    logs: Vec<DirectionalTradeLog>,
    close_all_position: bool,
}

pub struct DirectionalTrade {
    base_arbitrage: BaseArbitrage,
    config: DirectionalTradeConfig,
    state: DirectionalTradeState,
}

impl DirectionalTrade {
    pub fn new(
        amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
        gas: f64,
        short_trade_period: usize,
        medium_trade_period: usize,
        long_trade_period: usize,
        percentage_loss_threshold: f64,
        percentage_profit_threshold: f64,
        percentage_drop_threshold: f64,
        max_position_amount: f64,
        max_hold_period: usize,
        match_multiplier: f64,
        mismatch_multiplier: f64,
        log_limit: usize,
    ) -> Self {
        let config = DirectionalTradeConfig {
            short_trade_period,
            medium_trade_period,
            long_trade_period,
            percentage_loss_threshold,
            percentage_profit_threshold,
            percentage_drop_threshold,
            max_position_amount,
            max_hold_period,
            match_multiplier,
            mismatch_multiplier,
            log_limit,
        };

        let state = DirectionalTradeState {
            open_positions: HashMap::new(),
            logs: Vec::with_capacity(log_limit),
            close_all_position: false,
        };

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
            config,
            state,
        }
    }

    pub fn leverage(&self) -> f64 {
        self.base_arbitrage.leverage()
    }

    pub fn initial_amount(&self) -> f64 {
        self.base_arbitrage.initial_amount()
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
        for (_token_name, position) in &self.state.open_positions {
            amount += position.amount;
        }
        amount < self.config.max_position_amount
    }

    async fn get_current_prices(
        &self,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        // Get the prices of token pairs
        let token_pair_prices: HashMap<(String, String, String), f64> =
            self.get_token_pair_prices().await?;

        for ((token_a_name, token_b_name, dex), price) in &token_pair_prices {
            log::debug!(
                "Token pair price: {:<6}-{:<6}@{:<6}: {:12.6}",
                token_a_name,
                token_b_name,
                dex,
                price
            );

            // Update the price history and predict next prices
            if token_a_name == self.base_token().symbol_name() {
                let history = histories.entry(token_b_name.clone()).or_insert_with(|| {
                    PriceHistory::new(
                        self.config.short_trade_period,
                        self.config.medium_trade_period,
                        self.config.long_trade_period,
                        self.config.long_trade_period,
                        self.config.percentage_drop_threshold,
                    )
                });
                let buy_price = 1.0 / price;
                history.add_price(chrono::Utc::now().timestamp(), buy_price);
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

        if self.state.close_all_position {
            return Ok(opportunities);
        }

        for ((base_token_name, token_b_name, dex_name), buy_price) in current_prices {
            if base_token_name != self.base_token().symbol_name() {
                continue;
            }

            if let Some(history) = histories.get(token_b_name) {
                let price = 1.0 / buy_price; // sell price
                let predicted_price = history.majority_vote_predictions(
                    self.config.short_trade_period,
                    TradingStrategy::Contrarian,
                );
                let percentage_price_diff = (predicted_price - price) * 100.0 / price;

                log::debug!(
                    "{:<6}: {:6.3}%, current: {:6.3}, predict: {:6.3}",
                    token_b_name,
                    percentage_price_diff,
                    price,
                    predicted_price,
                );

                let amount = self.initial_amount() * self.leverage();

                if predicted_price > price {
                    if history.is_flash_crash() {
                        log::info!(
                            "Skip this buy trade as price of {} is crashed({:6.3} --> {:6.3})",
                            token_b_name,
                            price,
                            predicted_price
                        );
                        continue;
                    }

                    let profit = (predicted_price - price) * amount;

                    if percentage_price_diff > self.config.percentage_profit_threshold {
                        let token_a_index =
                            find_index(&self.tokens(), |token| token.symbol_name() == token_b_name)
                                .ok_or("Token not found")?;
                        let token_b_index = find_index(&self.tokens(), |token| {
                            token.symbol_name() == base_token_name
                        })
                        .ok_or("Token not found")?;
                        let dex_index = find_index(&self.dexes(), |dex| dex.name() == dex_name)
                            .ok_or("Dex not found")?;

                        opportunities.push(ArbitrageOpportunity {
                            dex_index: vec![dex_index],
                            token_index: vec![token_b_index, token_a_index],
                            amounts: vec![amount],
                            predicted_profit: Some(profit),
                            currect_price: Some(price),
                            predicted_price: Some(predicted_price),
                            gas: self.base_arbitrage.gas(),
                        });
                    }
                }
            }
        }

        Ok(opportunities)
    }

    fn find_sell_opportunities(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
        histories: &HashMap<String, PriceHistory>,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<ArbitrageOpportunity> = vec![];

        let current_time = chrono::Utc::now().timestamp() as usize;

        for ((token_a_name, base_token_name, dex_name), price) in current_prices {
            if base_token_name != self.base_token().symbol_name() {
                continue;
            }

            let token_a_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_a_name)
                    .ok_or("Token not found")?;
            let base_token_index = find_index(&self.tokens(), |token| {
                token.symbol_name() == base_token_name
            })
            .ok_or("Token not found")?;
            let dex_index =
                find_index(&self.dexes(), |dex| dex.name() == dex_name).ok_or("Dex not found")?;

            if let Some(history) = histories.get(token_a_name) {
                if let Some(position) = self.state.open_positions.get(token_a_name) {
                    let mut trade_amount = 0.0;

                    if history.is_flash_crash() || self.state.close_all_position {
                        log::info!(
                            "Close the position of {}, as its price{:.6} is crashed.",
                            token_a_name,
                            price
                        );
                        trade_amount = position.amount;
                    } else {
                        let holding_period = current_time - position.open_time;
                        let percentage_potential_profit =
                            (price - position.average_price) * 100.0 / price;
                        let percentage_ppotential_loss =
                            (position.average_price - price) * 100.0 / price;

                        if holding_period > self.config.max_hold_period {
                            log::info!("Close the position as it reaches the limit of hold period");
                            trade_amount = position.amount;
                        } else if percentage_potential_profit
                            > self.config.percentage_profit_threshold
                            || percentage_ppotential_loss < self.config.percentage_loss_threshold
                        {
                            trade_amount = self.initial_amount() * self.leverage() / *price;
                        }
                    }

                    let profit = (price - position.average_price) * trade_amount;

                    if trade_amount > 0.0 {
                        opportunities.push(ArbitrageOpportunity {
                            dex_index: vec![dex_index],
                            token_index: vec![token_a_index, base_token_index],
                            amounts: vec![position.amount],
                            predicted_profit: Some(profit),
                            currect_price: Some(*price),
                            predicted_price: None,
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
        // Get current prices
        log::trace!("get_current_prices-->");
        let current_prices: HashMap<(String, String, String), f64> =
            self.get_current_prices(histories).await?;
        log::trace!("<--get_current_prices");

        // Log the current positions
        for ((token_a_name, _base_token_name, _dex_namee), price) in &current_prices {
            if let Some(position) = self.state.open_positions.get(token_a_name) {
                position.print_info(&token_a_name, *price);
            }
        }

        let mut results: Vec<ArbitrageOpportunity> = vec![];

        let mut result_for_open = self.find_buy_opportunities(&current_prices, &histories)?;
        results.append(&mut result_for_open);

        let mut result_for_close = self.find_sell_opportunities(&current_prices, &histories)?;
        results.append(&mut result_for_close);

        Ok(results)
    }

    pub fn close_all_positions(&mut self) -> () {
        self.state.close_all_position = true;
    }

    pub fn is_close_all_positions(&self) -> bool {
        self.state.close_all_position
    }
}

#[async_trait]
impl Arbitrage for DirectionalTrade {
    async fn execute_transactions(
        &mut self,
        opportunities: &Vec<ArbitrageOpportunity>,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // todo: make concurrent
        for opportunity in opportunities {
            let token_a = &self.tokens()[opportunity.token_index[0]];
            let token_b = &self.tokens()[opportunity.token_index[1]];

            let token_pair = TokenPair::new(Arc::new(token_a.clone()), Arc::new(token_b.clone()));

            let amount_in = opportunity.amounts[0];
            let dex = &self.dexes()[opportunity.dex_index[0]];

            let mut amount_out = None;

            // execute swap operation
            if !self.base_arbitrage.skip_write() {
                amount_out = Some(
                    dex.swap_token(
                        &token_pair,
                        amount_in,
                        wallet_and_provider.clone(),
                        address,
                        deadline_secs,
                    )
                    .await?,
                );
            }

            // update positions
            let token_a_name = token_a.symbol_name();
            let token_b_name = token_b.symbol_name();
            let current_price = opportunity.currect_price.unwrap();

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

                let amount_out = match amount_out {
                    Some(amount) => amount,
                    None => {
                        let out = amount_in / current_price;
                        out
                    }
                };

                let average_price = amount_in / amount_out;

                if let Some(position) = self.state.open_positions.get_mut(token_b_name) {
                    // if there are already open positions for this token, update them
                    position.amount += amount_out;
                    position.average_price = (position.average_price * position.amount + amount_in)
                        / (position.amount + amount_out);
                    log::debug!(
                        "Updated open position for token: {}, amount: {}, average price: {:.6}",
                        token_b_name,
                        position.amount,
                        position.average_price
                    );
                } else {
                    // else, create a new position
                    let open_position = OpenPosition::new(average_price, amount_out);
                    log::debug!(
                        "Created new open position for token: {}, amount: {}, average price: {:.6}, open time: {}",
                        token_b_name,
                        open_position.amount,
                        open_position.average_price,
                        open_position.open_time,
                    );
                    self.state
                        .open_positions
                        .insert(token_b_name.to_owned(), open_position);
                }
            } else {
                // This is a sell operation

                let amount_out = match amount_out {
                    Some(amount) => amount,
                    None => {
                        let out = amount_in * current_price;
                        out
                    }
                };

                if let Some(position) = self.state.open_positions.get_mut(token_a_name) {
                    position.amount -= amount_in;

                    let average_price = amount_in / amount_out;
                    let pnl = (average_price - position.average_price) * amount_in;
                    log::info!("PNL = {:6.2}", pnl);

                    if position.amount <= 0.0 {
                        self.state.open_positions.remove(token_a_name); // If all of this token has been sold, remove it from the open positions
                        log::debug!(
                            "Sold all of token: {}. Removed from open positions.",
                            token_a_name
                        );
                    } else {
                        log::debug!(
                            "Updated open position for token: {}, amount: {}, average price: {:.6}",
                            token_a_name,
                            position.amount,
                            position.average_price
                        );
                    }
                }
            }
        }
        Ok(())
    }

    async fn init(
        &mut self,
        owner: Address,
        min_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_arbitrage.init(owner, min_amount).await
    }

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        // Get the prices of all token pairs
        let mut get_price_futures = Vec::new();
        let base_token = &self.base_token();
        let dexes = &self.dexes();
        let tokens = &self.tokens();
        let amount = self.initial_amount() * self.leverage();

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
