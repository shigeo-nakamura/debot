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
    dex_index: usize,
    slippage: f64,
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
        position_creation_inteval: u64,
        reward_multiplier: f64,
        penalty_multiplier: f64,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        dex_index: usize,
        slippage: f64,
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
            dex_index,
            slippage,
        };

        let name = format!("{}-{}-AlgoTrader", chain_name, dex_index);

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
            dex_index,
            short_trade_period,
            medium_trade_period,
            long_trade_period,
        );

        let db_handler = Arc::new(Mutex::new(DBHandler::new(
            db_client.clone(),
            transaction_log.clone(),
        )));

        let fund_managers: Vec<_> = fund_manager_configurations
            .into_iter()
            .map(
                |(
                    name,
                    strategy,
                    stop_loss_strategy,
                    period,
                    buy_signal,
                    take_profit,
                    cut_loss,
                    score,
                    days,
                    hours,
                )| {
                    let fund_name = format!("{}-{}-{}", chain_name, dexes[dex_index].name(), name);

                    let prev_score = scores.get(&fund_name);
                    let intial_score = match prev_score {
                        Some(prev_score) => prev_score,
                        None => &score,
                    };

                    state.scores.insert(fund_name.to_owned(), *intial_score);

                    FundManager::new(
                        &fund_name,
                        open_positions_map.get(&fund_name).cloned(),
                        strategy,
                        stop_loss_strategy,
                        period,
                        leverage,
                        initial_amount,
                        min_trading_amount,
                        *intial_score,
                        position_creation_inteval,
                        buy_signal,
                        take_profit,
                        cut_loss,
                        days,
                        hours,
                        db_handler.clone(),
                    )
                },
            )
            .collect();

        let mut total_fund_amount = 0.0;
        for fund_manager in fund_managers {
            total_fund_amount += fund_manager.amount();
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
                total_fund_amount,
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
            if token_b_name == self.base_token().symbol_name() {
                // sell price
                let history = histories
                    .entry(token_a_name.clone())
                    .or_insert_with(|| self.create_price_history());
                let price_point = history.add_price(*price, None);

                if self.base_trader.save_prices() {
                    self.base_trader
                        .db_handler()
                        .lock()
                        .await
                        .log_price(self.name(), token_a_name, price_point)
                        .await;
                }
            }
        }

        Ok(token_pair_prices)
    }

    fn is_price_impacted(
        token_name: &str,
        buy_price: f64,
        sell_price: f64,
        amount_in: f64,
        slippage: f64,
    ) -> bool {
        let amount_out = (buy_price * amount_in) * sell_price;
        if amount_out >= amount_in {
            return false;
        }

        let diff = amount_in - amount_out;
        if diff / amount_in > slippage {
            log::info!(
                "Price impact is high:{} amount_in = {:6.6}, amount_out = {:6.6}, diff = {:3.3}",
                token_name,
                amount_in,
                amount_out,
                diff
            );
            return true;
        }

        return false;
    }

    fn find_buy_opportunities(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeOpportunity> = vec![];

        for ((token_a_name, token_b_name, dex_name), price) in current_prices {
            if token_b_name != self.base_token().symbol_name() {
                continue;
            }

            let token_a_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_a_name)
                    .ok_or("Token not found")?;
            let token_b_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_b_name)
                    .ok_or("Token not found")?;
            let dex_index =
                find_index(&self.dexes(), |dex| dex.name() == dex_name).ok_or("Dex not found")?;

            for fund_manager in self.state.fund_manager_map.values() {
                let proposal = fund_manager.find_buy_opportunities(token_a_name, *price, histories);

                if let Some(opportunity) = proposal {
                    let key = (
                        self.base_token().symbol_name().to_owned(),
                        token_a_name.to_owned(),
                        self.dexes()[self.config.dex_index].name().to_owned(),
                    );
                    let buy_price = current_prices.get(&key);
                    if buy_price.is_none() {
                        continue;
                    }
                    if Self::is_price_impacted(
                        token_a_name,
                        *buy_price.unwrap(),
                        opportunity.price,
                        opportunity.amount,
                        self.config.slippage,
                    ) {
                        continue;
                    }

                    opportunities.push(TradeOpportunity {
                        dex_index: vec![dex_index],
                        token_index: vec![token_b_index, token_a_index],
                        amounts: vec![opportunity.amount],
                        operation: Operation::Buy,
                        predicted_profit: Some(opportunity.profit),
                        currect_price: Some(opportunity.price),
                        predicted_price: Some(opportunity.predicted_price),
                        trader_name: opportunity.fund_name.to_owned(),
                        reason_for_sell: None,
                    });
                }
            }
        }

        Ok(opportunities)
    }

    fn find_sell_opportunities(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeOpportunity> = vec![];

        for ((token_a_name, token_b_name, dex_name), price) in current_prices {
            if token_b_name != self.base_token().symbol_name() {
                continue;
            }

            let token_a_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_a_name)
                    .ok_or("Token not found")?;
            let token_b_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_b_name)
                    .ok_or("Token not found")?;
            let dex_index =
                find_index(&self.dexes(), |dex| dex.name() == dex_name).ok_or("Dex not found")?;

            for fund_manager in self.state.fund_manager_map.values() {
                let proposal = fund_manager.find_sell_opportunities(token_a_name, *price);

                if let Some(proposal) = proposal {
                    opportunities.push(TradeOpportunity {
                        dex_index: vec![dex_index],
                        token_index: vec![token_a_index, token_b_index],
                        amounts: vec![proposal.amount],
                        operation: Operation::Sell,
                        predicted_profit: Some(proposal.profit),
                        currect_price: Some(proposal.price),
                        predicted_price: None,
                        trader_name: proposal.fund_name.to_owned(),
                        reason_for_sell: proposal.reason_for_sell,
                    });
                }
            }
        }

        for fund_manager in self.state.fund_manager_map.values() {
            fund_manager.end_liquidate();
        }

        Ok(opportunities)
    }

    fn check_positions(&self, current_prices: &HashMap<(String, String, String), f64>) {
        for fund_manager in self.state.fund_manager_map.values() {
            fund_manager.check_positions(current_prices, self.base_token().symbol_name());
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
        let current_prices: HashMap<(String, String, String), f64> =
            self.get_current_prices(histories).await?;

        self.check_positions(&current_prices);

        let mut result_for_open = self.find_buy_opportunities(&current_prices, histories)?;
        results.append(&mut result_for_open);

        let mut result_for_close = self.find_sell_opportunities(&current_prices)?;
        results.append(&mut result_for_close);

        Ok(results)
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
        let base_token_amount = match self.get_amount_of_token(owner, &self.base_token()).await {
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
            self.state.amount = base_token_amount / self.dexes().len() as f64;
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
                    .update_position(is_buy_trade, None, token_b_name, amount_in, amount_out)
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
                    )
                    .await;

                if opportunity.predicted_profit.is_some() {
                    let multiplier = match opportunity.predicted_profit > Some(0.0) {
                        true => self.config.reward_multiplier,
                        false => self.config.penalty_multiplier,
                    };
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
        let dex = &self.dexes()[self.config.dex_index];
        let tokens = &self.tokens();
        let amount = self.state.amount;

        // Get prices with base token / each token and each token / base token
        let mut get_price_futures = Vec::new();
        let mut dex_get_price_futures = self
            .base_trader
            .get_token_pair_prices(&dex, base_token, tokens, amount)
            .await;
        get_price_futures.append(&mut dex_get_price_futures);

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

    async fn get_amount_of_token(
        &self,
        owner: Address,
        token: &Box<dyn Token>,
    ) -> Result<f64, Box<dyn Error + Send + Sync>> {
        self.base_trader.get_amount_of_token(owner, token).await
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
