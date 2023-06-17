// algorithm_trader.rs

use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use crate::dex::dex::TokenPair;
use crate::dex::Dex;
use crate::token::Token;
use crate::trade::find_index;
use crate::trade::AbstractTrader;
use crate::trade::TradingStrategy;

use async_trait::async_trait;
use ethers::prelude::{Provider, SignerMiddleware};
use ethers::providers::Http;
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::NonceManagerMiddleware;
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};

use super::abstract_trader::BaseTrader;
use super::FundManager;
use super::{PriceHistory, TradeOpportunity};

#[derive(Serialize, Deserialize)]
pub struct ForcastTraderLog {
    open_time: i64,
    close_time: i64,
    event_type: String, // "take profit", "loss cut", ""hold period over
    token: String,      // token against the base token
    average_price: f64,
    position_amount: f64,
    realized_pnl: f64, // realized profit or loss
}

pub struct ForcastTraderConfig {
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
    flash_crash_threshold: f64,
    max_hold_interval: u64,
    reward_multiplier: f64,
    penalty_multiplier: f64,
    log_limit: usize,
}

pub struct ForcastTraderState {
    close_all_position: bool,
    fund_manager_map: HashMap<String, FundManager>,
}

pub struct ForcastTrader {
    base_trader: BaseTrader,
    config: ForcastTraderConfig,
    state: ForcastTraderState,
    logs: Vec<ForcastTraderLog>,
}

impl ForcastTrader {
    pub fn new(
        leverage: f64,
        initial_amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
        gas: f64,
        short_trade_period: usize,
        medium_trade_period: usize,
        long_trade_period: usize,
        take_profit_threshold: f64,
        cut_loss_threshold: f64,
        flash_crash_threshold: f64,
        max_hold_interval: u64,
        position_creation_inteval: u64,
        reward_multiplier: f64,
        penalty_multiplier: f64,
        log_limit: usize,
        initial_score: f64,
    ) -> Self {
        let config = ForcastTraderConfig {
            short_trade_period,
            medium_trade_period,
            long_trade_period,
            flash_crash_threshold,
            max_hold_interval,
            reward_multiplier,
            penalty_multiplier,
            log_limit,
        };

        let mut state = ForcastTraderState {
            close_all_position: false,
            fund_manager_map: HashMap::new(),
        };

        let fund_managers = vec![
            FundManager::new(
                "trend-follow-short",
                TradingStrategy::TrendFollowing,
                short_trade_period,
                leverage,
                initial_amount,
                initial_score,
                position_creation_inteval,
                take_profit_threshold,
                cut_loss_threshold,
            ),
            // FundManager::new(
            //     "trend-follow-medium",
            //     TradingStrategy::TrendFollowing,
            //     medium_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "trend-follow-long",
            //     TradingStrategy::TrendFollowing,
            //     long_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "mean-reversion-short",
            //     TradingStrategy::MeanReversion,
            //     short_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "mean-reversion-medium",
            //     TradingStrategy::MeanReversion,
            //     medium_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "mean-reversion-long",
            //     TradingStrategy::MeanReversion,
            //     long_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "constrarian-short",
            //     TradingStrategy::Contrarian,
            //     short_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "constrarian-medium",
            //     TradingStrategy::Contrarian,
            //     medium_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
            // FundManager::new(
            //     "constrarian-long",
            //     TradingStrategy::Contrarian,
            //     long_trade_period,
            //     leverage,
            //     initial_amount,
            //     initial_score,
            //     position_creation_inteval,
            //     take_profit_threshold,
            //     cut_loss_threshold,
            // ),
        ];

        for fund_manager in fund_managers {
            state
                .fund_manager_map
                .insert(fund_manager.fund_name().to_owned(), fund_manager);
        }

        Self {
            base_trader: BaseTrader::new(
                "AlgoTrader".to_string(),
                leverage,
                initial_amount,
                allowance_factor,
                tokens,
                base_token,
                dexes,
                skip_write,
                gas,
            ),
            config,
            state,
            logs: Vec::with_capacity(log_limit),
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
                let history = histories.entry(token_a_name.clone()).or_insert_with(|| {
                    PriceHistory::new(
                        self.config.short_trade_period,
                        self.config.medium_trade_period,
                        self.config.long_trade_period,
                        self.config.long_trade_period,
                        self.config.flash_crash_threshold,
                    )
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
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeOpportunity> = vec![];

        if self.state.close_all_position {
            return Ok(opportunities);
        }

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
                if !fund_manager.can_create_new_position() {
                    continue;
                }

                let proposal = fund_manager.find_buy_opportunities(token_a_name, *price, histories);

                if let Some(opportunity) = proposal {
                    opportunities.push(TradeOpportunity {
                        dex_index: vec![dex_index],
                        token_index: vec![token_b_index, token_a_index],
                        amounts: vec![opportunity.amount],
                        predicted_profit: Some(opportunity.profit),
                        currect_price: Some(opportunity.price),
                        predicted_price: Some(opportunity.predicted_price),
                        trader_name: opportunity.fund_name.to_owned(),
                    });
                }
            }
        }

        Ok(opportunities)
    }

    fn find_sell_opportunities(
        &self,
        current_prices: &HashMap<(String, String, String), f64>,
        histories: &HashMap<String, PriceHistory>,
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
                let proposal = fund_manager.find_sell_opportunities(
                    token_a_name,
                    *price,
                    histories,
                    self.config.max_hold_interval,
                );

                if let Some(opportunity) = proposal {
                    opportunities.push(TradeOpportunity {
                        dex_index: vec![dex_index],
                        token_index: vec![token_a_index, token_b_index],
                        amounts: vec![opportunity.amount],
                        predicted_profit: Some(opportunity.profit),
                        currect_price: Some(opportunity.price),
                        predicted_price: None,
                        trader_name: opportunity.fund_name.to_owned(),
                    });
                }
            }
        }

        Ok(opportunities)
    }

    pub async fn find_opportunities(
        &self,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        // Get current prices
        let current_prices: HashMap<(String, String, String), f64> =
            self.get_current_prices(histories).await?;

        let mut results: Vec<TradeOpportunity> = vec![];

        let mut result_for_open = self.find_buy_opportunities(&current_prices, &histories)?;
        results.append(&mut result_for_open);

        let mut result_for_close = self.find_sell_opportunities(&current_prices, &histories)?;
        results.append(&mut result_for_close);

        Ok(results)
    }

    pub fn close_all_positions(&mut self) -> () {
        self.state.close_all_position = true;

        for fund_manager in self.state.fund_manager_map.values_mut() {
            fund_manager.close_all_positions();
        }
    }

    pub fn is_close_all_positions(&self) -> bool {
        self.state.close_all_position
    }

    pub fn rebalance(&self) -> () {
        log::debug!("rebalance");
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
        // todo: make concurrent
        for opportunity in opportunities {
            let token_a = &self.tokens()[opportunity.token_index[0]];
            let token_b = &self.tokens()[opportunity.token_index[1]];
            let token_pair = TokenPair::new(Arc::new(token_a.clone()), Arc::new(token_b.clone()));

            let amount_in = opportunity.amounts[0];
            let dex = &self.dexes()[opportunity.dex_index[0]];

            let mut amount_out = None;

            // execute swap operation
            if !self.base_trader.skip_write() {
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

            let buy_trade = token_a_name == self.base_token().symbol_name();

            let fund_manager = self
                .state
                .fund_manager_map
                .get_mut(&opportunity.trader_name)
                .unwrap();

            if buy_trade {
                let amount_out = match amount_out {
                    Some(amount) => amount,
                    None => amount_in / current_price,
                };

                fund_manager.update_position(buy_trade, token_b_name, amount_in, amount_out);
            } else {
                let amount_out = match amount_out {
                    Some(amount) => amount,
                    None => {
                        let out = amount_in * current_price;
                        out
                    }
                };

                fund_manager.update_position(buy_trade, token_a_name, amount_in, amount_out);
            }

            if opportunity.predicted_price.is_some() {
                let multiplier = match opportunity.predicted_profit > Some(0.0) {
                    true => self.config.reward_multiplier,
                    false => self.config.penalty_multiplier,
                };
                fund_manager.apply_reward_or_penalty(multiplier);
            }
        }

        self.rebalance();

        Ok(())
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
                .base_trader
                .get_token_pair_prices(dex, base_token, tokens, amount)
                .await;
            get_price_futures.append(&mut dex_get_price_futures);
        }

        // Wait for all token price futures to finish with a timeout
        log::debug!("call join_all");
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
        log::debug!("join_all done");

        let mut token_pair_prices = HashMap::new();

        // Aggregate all the prices into a HashMap
        for price_result in prices_results.into_iter() {
            match price_result {
                Ok(Ok(Some((token_1_symbol, token_2_symbol, dex_name, price)))) => {
                    token_pair_prices.insert((token_1_symbol, token_2_symbol, dex_name), price);
                }
                Ok(Ok(None)) => {
                    log::info!("No price info returned")
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
}
