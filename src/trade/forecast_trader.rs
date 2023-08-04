// algorithm_trader.rs

use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::dex::dex::TokenPair;
use crate::dex::Dex;
use crate::token::Token;
use crate::trade::find_index;
use crate::trade::AbstractTrader;

use async_trait::async_trait;
use ethers::prelude::{Provider, SignerMiddleware};
use ethers::providers::Http;
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::NonceManagerMiddleware;
use shared_mongodb::ClientHolder;
use tokio::time::{timeout, Duration};

use super::abstract_trader::BaseTrader;
use super::abstract_trader::TraderState;
use super::fund_configurations;
use super::DBHandler;
use super::FundManager;
use super::Operation;
use super::TradePosition;
use super::TransactionLog;
use super::{PriceHistory, TradeOpportunity};

pub struct DexPrice {
    pub dex_string: String,
    pub price: f64,
}

pub struct DexPrices {
    pub buy: DexPrice,
    pub sell: DexPrice,
    pub spread: f64,
    pub relative_spread: f64,
}

pub struct ForcastTraderConfig {
    chain_name: String,
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
    max_price_size: u32,
    interval: u64,
    flash_crash_threshold: f64,
    reward_multiplier: f64,
    penalty_multiplier: f64,
    spread: f64,
}

pub struct ForcastTraderState {
    amount: f64,
    fund_manager_map: HashMap<String, FundManager>,
    scores: HashMap<String, f64>,
}

pub struct ForcastTrader {
    base_trader: BaseTrader,
    config: ForcastTraderConfig,
    state: ForcastTraderState,
}

impl ForcastTrader {
    pub fn new(
        chain_name: &str,
        trader_state: HashMap<String, TraderState>,
        leverage: f64,
        initial_amount: f64,
        min_trading_amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        dry_run: bool,
        gas: f64,
        short_trade_period: usize,
        medium_trade_period: usize,
        long_trade_period: usize,
        max_price_size: u32,
        interval: u64,
        flash_crash_threshold: f64,
        position_creation_inteval_seconds: Option<u64>,
        reward_multiplier: f64,
        penalty_multiplier: f64,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        spread: f64,
        open_positions_map: HashMap<String, HashMap<String, TradePosition>>,
        prev_balance: Option<f64>,
        latest_scores: HashMap<String, HashMap<String, f64>>,
        save_prices: bool,
    ) -> Self {
        let config = ForcastTraderConfig {
            chain_name: chain_name.to_owned(),
            short_trade_period,
            medium_trade_period,
            long_trade_period,
            max_price_size,
            interval,
            flash_crash_threshold,
            reward_multiplier,
            penalty_multiplier,
            spread,
        };

        let name = format!("{}-AlgoTrader", chain_name);

        let binding = HashMap::new();
        let scores = match latest_scores.get(&name) {
            Some(scores) => scores,
            None => &binding,
        };

        let mut state = ForcastTraderState {
            amount: initial_amount,
            fund_manager_map: HashMap::new(),
            scores: scores.clone(),
        };

        let fund_manager_configurations = fund_configurations::get(
            chain_name,
            short_trade_period,
            medium_trade_period,
            long_trade_period,
        );

        let db_handler = Arc::new(Mutex::new(DBHandler::new(
            db_client.clone(),
            transaction_log.clone(),
        )));

        let amount_per_fund = initial_amount / fund_manager_configurations.len() as f64;

        let fund_managers: Vec<_> = fund_manager_configurations
            .into_iter()
            .filter_map(
                |(
                    name,
                    token_name,
                    strategy,
                    take_profit_strategy,
                    cut_loss_strategy,
                    period,
                    buy_signal,
                    take_profit,
                    cut_loss,
                    score,
                    days,
                    hours,
                )| {
                    let mut token_found = false;
                    for token in tokens.iter() {
                        if token_name == token.symbol_name() {
                            token_found = true;
                            break;
                        }
                    }

                    if !token_found {
                        return None;
                    }

                    let fund_name = format!("{}-{}-{}", chain_name, token_name, name);

                    let prev_score = scores.get(&fund_name);
                    let intial_score = match prev_score {
                        Some(prev_score) => prev_score,
                        None => &score,
                    };

                    state.scores.insert(fund_name.to_owned(), *intial_score);

                    Some(FundManager::new(
                        &fund_name,
                        token_name,
                        open_positions_map.get(&fund_name).cloned(),
                        strategy,
                        take_profit_strategy,
                        cut_loss_strategy,
                        period,
                        leverage,
                        amount_per_fund,
                        min_trading_amount,
                        *intial_score,
                        position_creation_inteval_seconds,
                        buy_signal,
                        take_profit,
                        cut_loss,
                        days,
                        hours,
                        db_handler.clone(),
                    ))
                },
            )
            .collect();

        for fund_manager in fund_managers {
            state
                .fund_manager_map
                .insert(fund_manager.name().to_owned(), fund_manager);
        }

        let trader_state = match trader_state.get(&name) {
            Some(state) => state,
            None => &TraderState::Active,
        };

        Self {
            base_trader: BaseTrader::new(
                name,
                trader_state.clone(),
                leverage,
                initial_amount,
                allowance_factor,
                tokens,
                base_token,
                dexes,
                dry_run,
                gas,
                db_handler,
                prev_balance,
                save_prices,
            ),
            config,
            state,
        }
    }

    async fn get_current_prices(
        &self,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<HashMap<String, DexPrices>, Box<dyn Error + Send + Sync>> {
        // Get the prices of token pairs
        let token_pair_prices = self.get_token_pair_prices().await?;

        let mut current_prices: HashMap<String, DexPrices> = HashMap::new();

        for token in self.tokens().iter() {
            let token_name = token.symbol_name();
            if token_name == self.base_token().symbol_name() {
                continue;
            }

            let buy_price = self.get_best_buy_price(token_name, &token_pair_prices);
            let sell_price = self.get_best_sell_price(token_name, &token_pair_prices);
            if buy_price.is_none() || sell_price.is_none() {
                continue;
            }

            let (buy_price, buy_dex_name) = buy_price.unwrap();
            let (sell_price, sell_dex_name) = sell_price.unwrap();

            // Calculate spread
            let key = token_name.to_string();
            let spread = buy_price - sell_price;

            // Create new DexPrices
            let new_prices = DexPrices {
                buy: DexPrice {
                    dex_string: buy_dex_name.to_owned(),
                    price: buy_price,
                },
                sell: DexPrice {
                    dex_string: sell_dex_name.to_string(),
                    price: sell_price,
                },
                spread: spread,
                relative_spread: spread / sell_price,
            };

            current_prices.insert(key, new_prices);
        }

        // Update the price history and predict next prices
        for (token_name, prices) in &current_prices {
            // Update the price history and predict next prices
            let history = histories
                .entry(token_name.clone())
                .or_insert_with(|| self.create_price_history());
            let price_point = history.add_price(prices.sell.price, None);

            if self.base_trader.save_prices() {
                self.base_trader
                    .db_handler()
                    .lock()
                    .await
                    .log_price(self.name(), token_name, price_point)
                    .await;
            }
        }

        Ok(current_prices)
    }

    fn get_best_price<F, C>(
        &self,
        token_name: &str,
        current_prices: &HashMap<(String, String, String), f64>,
        price_func: F,
        cmp: C,
    ) -> Option<(f64, String)>
    where
        F: Fn(&str, &HashMap<(String, String, String), f64>, &str) -> Option<f64>,
        C: Fn(f64, f64) -> bool,
    {
        let mut best_price: Option<(f64, String)> = None;

        for dex in self.dexes().iter() {
            let price = price_func(token_name, current_prices, &dex.name());

            best_price = match (best_price, price) {
                (Some((best, dex_name)), Some(current)) => {
                    if cmp(current, best) {
                        Some((current, dex.name().to_owned()))
                    } else {
                        Some((best, dex_name))
                    }
                }
                (None, Some(current)) => Some((current, dex.name().to_owned())),
                (Some((best, dex_name)), None) => Some((best, dex_name)),
                (None, None) => None,
            };
        }

        best_price
    }

    fn get_best_buy_price(
        &self,
        token_name: &str,
        current_prices: &HashMap<(String, String, String), f64>,
    ) -> Option<(f64, String)> {
        self.get_best_price(
            token_name,
            current_prices,
            |token_name, current_prices, dex_name| {
                let key = (
                    self.base_token().symbol_name().to_owned(),
                    token_name.to_owned(),
                    dex_name.to_owned(),
                );
                self.get_price(current_prices, &key, true)
            },
            |current, best| current < best,
        )
    }

    fn get_best_sell_price(
        &self,
        token_name: &str,
        current_prices: &HashMap<(String, String, String), f64>,
    ) -> Option<(f64, String)> {
        self.get_best_price(
            token_name,
            current_prices,
            |token_name, current_prices, dex_name| {
                let key = (
                    token_name.to_owned(),
                    self.base_token().symbol_name().to_owned(),
                    dex_name.to_owned(),
                );
                self.get_price(current_prices, &key, false)
            },
            |current, best| current > best,
        )
    }

    fn get_price(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
        key: &(String, String, String),
        invert: bool,
    ) -> Option<f64> {
        match current_prices.get(key).copied() {
            Some(price) => {
                if price == 0.0 {
                    None
                } else {
                    if invert {
                        Some(1.0 / price)
                    } else {
                        Some(price)
                    }
                }
            }
            None => None,
        }
    }

    fn is_wide_spread(prices: &DexPrices, relative_spread: f64) -> bool {
        prices.relative_spread > relative_spread
    }

    fn get_token_index(&self, token_name: &str) -> Result<usize, Box<dyn Error + Send + Sync>> {
        let index = find_index(&self.tokens(), |token| token.symbol_name() == token_name)
            .ok_or("Token not found")?;
        Ok(index)
    }

    fn get_dex_index(&self, dex_name: &str) -> Result<usize, Box<dyn Error + Send + Sync>> {
        let index =
            find_index(&self.dexes(), |dex| dex.name() == dex_name).ok_or("Dex not found")?;
        Ok(index)
    }

    fn find_buy_opportunities(
        &self,
        current_prices: &HashMap<String, DexPrices>,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeOpportunity> = vec![];

        for (token_name, prices) in current_prices {
            let token_a_index = self.get_token_index(token_name)?;
            let token_b_index = self.get_token_index(self.base_token().symbol_name())?;
            let dex_index = self.get_dex_index(&prices.buy.dex_string)?;

            // Check if the prices are reliable
            if Self::is_wide_spread(prices, self.config.spread) {
                continue;
            }

            for fund_manager in self.state.fund_manager_map.values() {
                let proposal = fund_manager.find_buy_opportunities(
                    token_name,
                    prices.buy.price,
                    prices.sell.price,
                    histories,
                );

                if let Some(proposal) = proposal {
                    opportunities.push(TradeOpportunity {
                        dex_index: vec![dex_index],
                        token_index: vec![token_b_index, token_a_index],
                        amounts: vec![proposal.amount],
                        operation: Operation::Buy,
                        predicted_profit: Some(proposal.profit),
                        currect_price: Some(proposal.execution_price),
                        predicted_price: proposal.predicted_price,
                        trader_name: proposal.fund_name.to_owned(),
                        reason_for_sell: None,
                        atr: proposal.atr,
                    });
                }
            }
        }

        Ok(opportunities)
    }

    fn find_sell_opportunities(
        &self,
        current_prices: &HashMap<String, DexPrices>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeOpportunity> = vec![];

        for (token_name, prices) in current_prices {
            let token_a_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_name)
                    .ok_or("Token not found")?;
            let token_b_index = find_index(&self.tokens(), |token| {
                token.symbol_name() == self.base_token().symbol_name()
            })
            .ok_or("Token not found")?;
            let dex_index = find_index(&self.dexes(), |dex| dex.name() == prices.sell.dex_string)
                .ok_or("Dex not found")?;

            for fund_manager in self.state.fund_manager_map.values() {
                let proposal = fund_manager.find_sell_opportunities(token_name, prices.sell.price);

                if let Some(proposal) = proposal {
                    opportunities.push(TradeOpportunity {
                        dex_index: vec![dex_index],
                        token_index: vec![token_a_index, token_b_index],
                        amounts: vec![proposal.amount],
                        operation: Operation::Sell,
                        predicted_profit: Some(proposal.profit),
                        currect_price: Some(proposal.execution_price),
                        predicted_price: proposal.predicted_price,
                        trader_name: proposal.fund_name.to_owned(),
                        reason_for_sell: proposal.reason_for_sell,
                        atr: None,
                    });
                }
            }
        }

        for fund_manager in self.state.fund_manager_map.values() {
            fund_manager.end_liquidate();
        }

        Ok(opportunities)
    }

    pub async fn find_opportunities(
        &self,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut results: Vec<TradeOpportunity> = vec![];

        if self.base_trader.state() != TraderState::Active {
            log::warn!("{}'s state {:?}", self.name(), self.state());
            return Ok(results);
        }

        // Get current prices
        let current_prices: HashMap<String, DexPrices> = self.get_current_prices(histories).await?;

        for (token_name, prices) in &current_prices {
            log::debug!(
                "current price: {:<6}: {:12.6}@{:<12} - {:12.6}@{:<12} ({:3.3})%",
                token_name,
                prices.sell.price,
                prices.sell.dex_string,
                prices.buy.price,
                prices.buy.dex_string,
                prices.relative_spread * 100.0,
            );
        }

        self.check_positions(&current_prices);

        let mut result_for_open = self.find_buy_opportunities(&current_prices, histories)?;
        results.append(&mut result_for_open);

        let mut result_for_close = self.find_sell_opportunities(&current_prices)?;
        results.append(&mut result_for_close);

        Ok(results)
    }

    fn check_positions(&self, current_prices: &HashMap<String, DexPrices>) {
        for fund_manager in self.state.fund_manager_map.values() {
            fund_manager.check_positions(current_prices);
        }
    }

    fn total_fund_amount(&self) -> f64 {
        let mut amount = 0.0;
        for fund_manager in self.state.fund_manager_map.values() {
            amount += fund_manager.amount();
        }
        amount
    }

    pub fn is_any_fund_liquidated(&self) -> bool {
        for fund_manager in self.state.fund_manager_map.values() {
            if fund_manager.is_liquidated() {
                return true;
            }
        }
        false
    }

    pub async fn pause(&mut self) {
        if self.state() == TraderState::Active {
            self.set_state(TraderState::Paused).await;
        }
    }

    pub async fn liquidate(&mut self, chain_name: &str) {
        for fund_manager in self.state.fund_manager_map.values_mut() {
            fund_manager.begin_liquidate();
        }

        self.base_trader.set_state(TraderState::Liquidated).await;
        self.base_trader
            .db_handler()
            .lock()
            .await
            .log_liquidate_time(chain_name)
            .await;
    }

    pub async fn rebalance(&mut self, owner: Address, force_rebalance: bool) {
        let base_token_amount =
            match BaseTrader::get_amount_of_token(owner, &self.base_token()).await {
                Ok(amount) => amount,
                Err(e) => {
                    log::error!("rebalance failed: {:?}", e);
                    return;
                }
            };

        if base_token_amount == 0.0 {
            log::warn!("rebalance: No balance left in {}", self.config.chain_name);
            return;
        }

        if self.base_trader.dry_run() {
            self.state.amount = self.total_fund_amount();
        } else {
            self.state.amount = base_token_amount;
        }

        log::info!(
            "{}: available amount = {:6.3}",
            self.name(),
            self.state.amount
        );

        let mut total_score = 0.0;
        let mut changed = false;

        for fund_manager in self.state.fund_manager_map.values() {
            let score = fund_manager.score();
            let name = fund_manager.name();
            total_score += score;

            let prev_score = self.state.scores.insert(name.to_owned(), score);
            if prev_score.is_some() {
                if score != prev_score.unwrap() {
                    changed = true;
                }
            }
        }

        if changed {
            log::info!("rebalanced scores: {:?}", self.state.scores);

            self.base_trader
                .db_handler()
                .lock()
                .await
                .log_performance(self.name(), self.state.scores.clone())
                .await;
        }

        if !changed && !force_rebalance {
            return;
        }

        let amount_per_score = self.state.amount / total_score;

        for fund_manager in self.state.fund_manager_map.values_mut() {
            let amount = match fund_manager.is_liquidated() {
                true => 0.0,
                false => fund_manager.score() * amount_per_score,
            };
            fund_manager.set_amount(amount);
        }
    }

    pub fn create_price_history(&self) -> PriceHistory {
        PriceHistory::new(
            self.config.short_trade_period,
            self.config.medium_trade_period,
            self.config.long_trade_period,
            self.config.max_price_size as usize,
            self.config.interval,
            self.config.flash_crash_threshold,
        )
    }
}

#[async_trait]
impl AbstractTrader for ForcastTrader {
    async fn execute_transactions(
        &mut self,
        opportunities: &Vec<TradeOpportunity>,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if opportunities.is_empty() {
            return Ok(());
        }

        // Group opportunities by token and dex
        let mut opportunity_groups: HashMap<(usize, [usize; 2]), Vec<TradeOpportunity>> =
            HashMap::new();

        for opportunity in opportunities {
            let mut sorted_token_index = opportunity.token_index.clone();
            sorted_token_index.sort_unstable();

            // dex_index should have exactly one element
            let dex_index = opportunity.dex_index[0];

            // sorted_token_index should have exactly two elements
            let token_index = [sorted_token_index[0], sorted_token_index[1]];

            let key = (dex_index, token_index);

            opportunity_groups
                .entry(key)
                .or_default()
                .push(opportunity.clone());
        }

        // Process each group as a single transaction
        let mut swap_token_futures = vec![];
        for ((dex_index, token_index), opportunities) in opportunity_groups {
            let token_a = &self.tokens()[token_index[0]].clone();
            let token_b = &self.tokens()[token_index[1]].clone();
            let token_pair = TokenPair::new(Arc::new(token_a.clone()), Arc::new(token_b.clone()));
            let dex = &self.dexes()[dex_index];

            // calculate total amount for the group
            let (total_buy_amount, total_sell_amount): (f64, f64) =
                opportunities
                    .iter()
                    .fold((0.0, 0.0), |acc, o| match o.operation {
                        Operation::Buy => (acc.0 + o.amounts[0], acc.1),
                        Operation::Sell => (acc.0, acc.1 + o.amounts[0]),
                    });
            // calculate net amount
            let net_amount = total_buy_amount - total_sell_amount;

            // execute swap operation
            if !self.base_trader.dry_run() {
                let new_token_pair = match net_amount > 0.0 {
                    true => token_pair,
                    false => token_pair.swap(),
                };
                let dex_clone = dex.clone();
                let future = async move {
                    dex_clone
                        .swap_token(
                            &new_token_pair,
                            net_amount.abs(),
                            wallet_and_provider.clone(),
                            address,
                            deadline_secs,
                        )
                        .await
                };
                swap_token_futures.push(future);
            }
        }

        // Wait for all token price futures to finish with a timeout
        let timeout_duration = Duration::from_secs(10);
        let swap_token_results = timeout(
            timeout_duration,
            futures::future::join_all(swap_token_futures),
        )
        .await;

        match swap_token_results {
            Ok(values) => {
                for value in values.into_iter() {
                    match value {
                        Ok(amount_out) => {
                            log::trace!("swap_token result = {:6.3}", amount_out);
                        }
                        Err(e) => {
                            log::error!("swap_token result: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                // Handle the join error case
                return Err(Box::new(e) as Box<dyn Error + Send + Sync>);
            }
        }

        // update the position of each fund managers

        for opportunity in opportunities {
            let token_a = &self.tokens()[opportunity.token_index[0]];
            let token_b = &self.tokens()[opportunity.token_index[1]];
            let amount_in = opportunity.amounts[0];
            let atr = opportunity.atr;

            let token_a_name = token_a.symbol_name();
            let token_b_name = token_b.symbol_name();
            let current_price = opportunity.currect_price.unwrap();

            let is_buy_trade = opportunity.operation == Operation::Buy;

            let fund_manager = self
                .state
                .fund_manager_map
                .get_mut(&opportunity.trader_name)
                .unwrap();

            if is_buy_trade {
                let amount_out = amount_in / current_price;
                fund_manager
                    .update_position(is_buy_trade, None, token_b_name, amount_in, amount_out, atr)
                    .await;
            } else {
                let amount_out = amount_in * current_price;
                fund_manager
                    .update_position(
                        is_buy_trade,
                        opportunity.reason_for_sell.clone(),
                        token_a_name,
                        amount_in,
                        amount_out,
                        atr,
                    )
                    .await;

                if opportunity.predicted_profit.is_some() {
                    let multiplier;
                    match opportunity.predicted_profit {
                        Some(profit) => {
                            let ratio = profit / opportunity.amounts[0];
                            multiplier = match ratio {
                                _ if ratio > 0.01 => self.config.reward_multiplier,
                                _ if ratio < -0.01 => self.config.penalty_multiplier,
                                _ => 1.0,
                            };
                        }
                        None => {
                            multiplier = 1.0;
                        }
                    }
                    fund_manager.apply_reward_or_penalty(multiplier);
                }
            }
        }

        self.rebalance(address, false).await;

        Ok(())
    }

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        let base_token = &self.base_token();
        let dexes = &self.dexes();
        let tokens = &self.tokens();
        let amount = self.base_trader.initial_amount();

        // Get prices with base token / each token and each token / base token
        let mut get_price_futures = Vec::new();
        for dex in dexes.iter() {
            let mut dex_get_price_futures = self
                .base_trader
                .get_token_pair_prices(&dex, base_token, tokens, amount)
                .await;
            get_price_futures.append(&mut dex_get_price_futures);
        }

        // Wait for all token price futures to finish with a timeout
        let timeout_duration = Duration::from_secs(10);
        let prices_results = timeout(
            timeout_duration,
            futures::future::join_all(get_price_futures),
        )
        .await;

        // Check if timeout occurred
        let prices_results = match prices_results {
            Ok(results) => results, // On success, get the results
            Err(_) => return Err("Timeout occurred".into()), // On timeout, return an error
        };

        let mut token_pair_prices = HashMap::new();

        // Aggregate all the prices into a HashMap
        for price_result in prices_results.into_iter() {
            match price_result {
                Ok(Ok(Some((token_1_symbol, token_2_symbol, dex_name, price)))) => {
                    token_pair_prices.insert((token_1_symbol, token_2_symbol, dex_name), price);
                }
                Ok(Ok(None)) => {
                    continue;
                }
                Ok(Err(e)) => {
                    // Handle the error case
                    return Err(e);
                }
                Err(e) => {
                    // Handle the join error case
                    return Err(Box::new(e) as Box<dyn Error + Send + Sync>);
                }
            }
        }

        Ok(token_pair_prices)
    }

    fn state(&self) -> TraderState {
        self.base_trader.state()
    }

    async fn set_state(&mut self, state: TraderState) {
        self.base_trader.set_state(state).await;
    }

    fn leverage(&self) -> f64 {
        self.base_trader.leverage()
    }

    fn initial_amount(&self) -> f64 {
        self.base_trader.initial_amount()
    }

    fn tokens(&self) -> Arc<Vec<Box<dyn Token>>> {
        self.base_trader.tokens()
    }

    fn base_token(&self) -> Arc<Box<dyn Token>> {
        self.base_trader.base_token()
    }

    fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>> {
        self.base_trader.dexes()
    }

    fn name(&self) -> &str {
        self.base_trader.name()
    }

    fn db_handler(&self) -> &Arc<Mutex<DBHandler>> {
        self.base_trader.db_handler()
    }

    async fn init(
        &mut self,
        owner: Address,
        min_managed_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_trader.init(owner, min_managed_amount).await
    }

    async fn transfer_token(
        &self,
        recipient: Address,
        token: &Box<dyn Token>,
        amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_trader
            .transfer_token(recipient, token, amount)
            .await
    }

    async fn calculate_and_log_balance(
        &mut self,
        chain_name: &str,
        wallet_address: &Address,
    ) -> Option<f64> {
        self.base_trader
            .calculate_and_log_balance(chain_name, wallet_address)
            .await
    }
}
