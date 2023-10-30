// algorithm_trader.rs

use chrono::Datelike;
use chrono::Local;
use chrono::Weekday;
use debot_ether_utils::dex::dex::TokenPair;
use debot_ether_utils::Dex;
use debot_ether_utils::Token;
use debot_market_analyzer::MarketData;
use debot_position_manager::ReasonForClose;
use debot_position_manager::TradeAction;
use debot_position_manager::TradeChance;
use debot_position_manager::TradePosition;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

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
use super::fund_config;
use super::fund_config::TradingStyle;
use super::DBHandler;
use super::FundManager;
use super::TransactionLog;

#[derive(Clone)]
pub struct TradingPeriod {
    short_term_hour: usize,
    medium_term_hour: usize,
    long_term_hour: usize,
}

impl TradingPeriod {
    pub fn new(short_term_hour: usize, medium_term_hour: usize, long_term_hour: usize) -> Self {
        Self {
            short_term_hour,
            medium_term_hour,
            long_term_hour,
        }
    }
}

#[derive(Clone)]
pub struct DexPrice {
    pub dex_string: String,
    pub price: f64,
}

#[derive(Clone)]
pub struct DexPrices {
    pub buy: DexPrice,
    pub sell: DexPrice,
    pub spread: f64,
    pub relative_spread: f64,
}

pub struct ForcastTraderConfig {
    master: bool,
    chain_name: String,
    trading_style: TradingStyle,
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
    max_price_size: u32,
    interval: u64,
    spread: f64,
    position_creation_inteval_seconds: Option<u64>,
}

pub struct ForcastTraderState {
    amount: f64,
    fund_manager_map: HashMap<String, FundManager>,
    current_prices: Arc<std::sync::Mutex<HashMap<String, DexPrices>>>,
    pub last_loss_cut_time: i64,
}

pub struct ForcastTrader {
    base_trader: BaseTrader,
    config: ForcastTraderConfig,
    state: ForcastTraderState,
}

impl ForcastTrader {
    pub fn new(
        master: bool,
        index: usize,
        current_prices: Arc<std::sync::Mutex<HashMap<String, DexPrices>>>,
        chain_name: &str,
        trader_state: HashMap<String, TraderState>,
        initial_amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        anchor_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        dry_run: bool,
        gas: f64,
        trading_period: TradingPeriod,
        max_price_size: u32,
        interval: u64,
        risk_reward: f64,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        spread: f64,
        open_positions_map: HashMap<String, HashMap<String, TradePosition>>,
        prev_balance: Option<f64>,
        save_prices: bool,
        position_creation_inteval_seconds: Option<u64>,
    ) -> Self {
        let target_trading_style = if index == 0 {
            TradingStyle::Day
        } else if index == 1 {
            TradingStyle::Swing
        } else {
            panic!("Unknown trayding style");
        };

        let config = ForcastTraderConfig {
            master,
            chain_name: chain_name.to_owned(),
            trading_style: target_trading_style.clone(),
            short_trade_period: trading_period.short_term_hour * 3600 / interval as usize,
            medium_trade_period: trading_period.medium_term_hour * 3600 / interval as usize,
            long_trade_period: trading_period.long_term_hour * 3600 / interval as usize,
            max_price_size,
            interval,
            spread,
            position_creation_inteval_seconds,
        };

        let name = format!("{}-Algo-{}", chain_name, target_trading_style.to_string());

        let mut state = ForcastTraderState {
            amount: initial_amount,
            fund_manager_map: HashMap::new(),
            current_prices,
            last_loss_cut_time: 0,
        };

        let fund_manager_configurations = fund_config::get(target_trading_style, chain_name);

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
                    trading_style,
                    token_name,
                    strategy,
                    period_hour,
                    activity_time,
                    buy_signal,
                    trading_amount,
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

                    let fund_name =
                        format!("{}-{}-{}-{}", chain_name, trading_style, token_name, name);

                    let period = period_hour * 60 * 60 / (interval as usize);

                    Some(FundManager::new(
                        &fund_name,
                        &token_name,
                        open_positions_map.get(&fund_name).cloned(),
                        strategy,
                        period,
                        activity_time,
                        trading_amount,
                        amount_per_fund,
                        buy_signal,
                        period as u64 * interval,
                        risk_reward,
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
                initial_amount,
                allowance_factor,
                tokens,
                anchor_token,
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

    pub fn chain_name(&self) -> &str {
        &self.config.chain_name
    }

    async fn get_current_prices(
        &self,
        amount: f64,
        market_data: &mut HashMap<String, MarketData>,
    ) -> Result<HashMap<String, DexPrices>, Box<dyn Error + Send + Sync>> {
        let mut current_prices: HashMap<String, DexPrices> = HashMap::new();

        if self.config.master {
            // Get the prices of token pairs
            let token_pair_prices = self.get_token_pair_prices(amount).await?;

            for token in self.tokens().iter() {
                let token_name = token.symbol_name();
                if token_name == self.anchor_token().symbol_name() {
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
                let mut prices = self.state.current_prices.lock().unwrap();
                *prices = current_prices.clone();
            }
        } else {
            current_prices = self.state.current_prices.lock().unwrap().clone();
        }

        // Update the market data and predict next prices
        for (token_name, prices) in &current_prices {
            let data = market_data
                .entry(token_name.clone())
                .or_insert_with(|| self.create_market_data());
            let price_point = data.add_price(prices.sell.price, None);

            if self.config.master && self.base_trader.save_prices() {
                self.base_trader
                    .db_handler()
                    .lock()
                    .await
                    .log_price(data.name(), token_name, price_point)
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
                    self.anchor_token().symbol_name().to_owned(),
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
                    self.anchor_token().symbol_name().to_owned(),
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

    fn is_forbidden_to_buy(&self) -> bool {
        let now = Local::now();
        let is_forbidden = match now.weekday() {
            Weekday::Fri => {
                if self.config.trading_style == TradingStyle::Swing {
                    true
                } else {
                    false
                }
            }
            Weekday::Sat => true,
            Weekday::Sun => true,
            _ => false,
        };
        if is_forbidden {
            return true;
        }

        if let Some(interval_seconds) = self.config.position_creation_inteval_seconds {
            let current_time = chrono::Utc::now().timestamp();
            let elapsed_time = current_time - self.state.last_loss_cut_time;
            if elapsed_time < interval_seconds.try_into().unwrap() {
                return true;
            }
        }

        return false;
    }

    fn find_buy_opportunities(
        &self,
        amount: f64,
        current_prices: &HashMap<String, DexPrices>,
        market_data: &mut HashMap<String, MarketData>,
    ) -> Result<Vec<TradeChance>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeChance> = vec![];

        if self.is_forbidden_to_buy() {
            return Ok(opportunities);
        }

        for (token_name, prices) in current_prices {
            let token_a_index = self.get_token_index(token_name)?;
            let token_b_index = self.get_token_index(self.anchor_token().symbol_name())?;
            let dex_index = self.get_dex_index(&prices.buy.dex_string)?;

            // Check if the prices are reliable
            if prices.relative_spread > self.config.spread {
                continue;
            }

            for fund_manager in self.state.fund_manager_map.values() {
                let proposal = fund_manager.find_buy_opportunities(
                    token_name,
                    prices.buy.price,
                    prices.sell.price,
                    prices.relative_spread,
                    amount,
                    market_data,
                );

                if let Some(proposal) = proposal {
                    opportunities.push(TradeChance {
                        dex_index: vec![dex_index],
                        token_index: vec![token_b_index, token_a_index],
                        amounts: proposal.amounts,
                        action: TradeAction::BuyOpen,
                        current_price: proposal.current_price,
                        predicted_price: proposal.predicted_price,
                        trader_name: proposal.trader_name.to_owned(),
                        reason_for_close: None,
                        atr: proposal.atr,
                        momentum: proposal.momentum,
                    });
                }
            }
        }

        Ok(opportunities)
    }

    fn find_sell_opportunities(
        &self,
        current_prices: &HashMap<String, DexPrices>,
        market_data: &mut HashMap<String, MarketData>,
    ) -> Result<Vec<TradeChance>, Box<dyn Error + Send + Sync>> {
        let mut opportunities: Vec<TradeChance> = vec![];

        for (token_name, prices) in current_prices {
            let token_a_index =
                find_index(&self.tokens(), |token| token.symbol_name() == token_name)
                    .ok_or("Token not found")?;
            let token_b_index = find_index(&self.tokens(), |token| {
                token.symbol_name() == self.anchor_token().symbol_name()
            })
            .ok_or("Token not found")?;
            let dex_index = find_index(&self.dexes(), |dex| dex.name() == prices.sell.dex_string)
                .ok_or("Dex not found")?;

            // Check if the prices are reliable
            let limited_sell = prices.relative_spread > self.config.spread;

            for fund_manager in self.state.fund_manager_map.values() {
                let proposal = fund_manager.find_sell_opportunities(
                    token_name,
                    prices.sell.price,
                    limited_sell,
                    market_data,
                );

                if let Some(proposal) = proposal {
                    opportunities.push(TradeChance {
                        dex_index: vec![dex_index],
                        token_index: vec![token_a_index, token_b_index],
                        amounts: proposal.amounts,
                        action: TradeAction::SellClose,
                        current_price: proposal.current_price,
                        predicted_price: proposal.predicted_price,
                        trader_name: proposal.trader_name.to_owned(),
                        reason_for_close: proposal.reason_for_close,
                        atr: None,
                        momentum: None,
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
        amount: f64,
        market_data: &mut HashMap<String, MarketData>,
    ) -> Result<Vec<TradeChance>, Box<dyn Error + Send + Sync>> {
        let mut results: Vec<TradeChance> = vec![];

        if self.base_trader.state() != TraderState::Active {
            log::warn!("{}'s state {:?}", self.name(), self.state());
            return Ok(results);
        }

        // Get current prices
        let current_prices: HashMap<String, DexPrices> =
            self.get_current_prices(amount, market_data).await?;

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

        let mut result_for_open =
            self.find_buy_opportunities(amount, &current_prices, market_data)?;
        results.append(&mut result_for_open);

        let mut result_for_close = self.find_sell_opportunities(&current_prices, market_data)?;
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

    pub async fn rebalance(&mut self, owner: Address) {
        let anchor_token_amount =
            match BaseTrader::get_amount_of_token(owner, &self.anchor_token()).await {
                Ok(amount) => amount,
                Err(e) => {
                    log::error!("rebalance failed: {:?}", e);
                    return;
                }
            };

        if anchor_token_amount == 0.0 {
            log::warn!("rebalance: No balance left in {}", self.config.chain_name);
            return;
        }

        if self.base_trader.dry_run() {
            self.state.amount = self.total_fund_amount();
        } else {
            self.state.amount = anchor_token_amount;
        }

        log::info!(
            "{}: available amount = {:6.3}",
            self.name(),
            self.state.amount
        );

        let amount_per_fund = self.state.amount / self.state.fund_manager_map.len() as f64;

        for fund_manager in self.state.fund_manager_map.values_mut() {
            let amount = match fund_manager.is_liquidated() {
                true => 0.0,
                false => amount_per_fund,
            };
            fund_manager.set_amount(amount);
        }
    }

    pub fn create_market_data(&self) -> MarketData {
        MarketData::new(
            self.config.chain_name.to_owned(),
            self.config.short_trade_period,
            self.config.medium_trade_period,
            self.config.long_trade_period,
            self.config.max_price_size as usize,
            self.config.interval,
        )
    }
}

#[async_trait]
impl AbstractTrader for ForcastTrader {
    async fn execute_transactions(
        &mut self,
        opportunities: &Vec<TradeChance>,
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
        let mut opportunity_groups: HashMap<(usize, [usize; 2]), Vec<TradeChance>> = HashMap::new();

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
                    .fold((0.0, 0.0), |acc, o| match o.action {
                        TradeAction::BuyOpen => (acc.0 + o.amounts[0], acc.1),
                        TradeAction::SellClose => (acc.0, acc.1 + o.amounts[0]),
                        _ => panic!("Not implemented"),
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
            let momentum = opportunity.momentum;

            let token_a_name = token_a.symbol_name();
            let token_b_name = token_b.symbol_name();
            let current_price = opportunity.current_price.unwrap();

            let is_buy_trade = opportunity.action == TradeAction::BuyOpen;

            let fund_manager = self
                .state
                .fund_manager_map
                .get_mut(&opportunity.trader_name)
                .unwrap();

            if is_buy_trade {
                let amount_out = amount_in / current_price;
                fund_manager
                    .update_position(
                        is_buy_trade,
                        None,
                        token_b_name,
                        amount_in,
                        amount_out,
                        atr,
                        momentum,
                        opportunity.predicted_price,
                    )
                    .await;
            } else {
                let amount_out = amount_in * current_price;
                fund_manager
                    .update_position(
                        is_buy_trade,
                        opportunity.reason_for_close.clone(),
                        token_a_name,
                        amount_in,
                        amount_out,
                        atr,
                        momentum,
                        None,
                    )
                    .await;

                match opportunity.reason_for_close {
                    Some(ReasonForClose::CutLoss) => {
                        self.state.last_loss_cut_time = chrono::Utc::now().timestamp()
                    }
                    _ => {}
                }
            }
        }

        // self.rebalance(address).await;

        Ok(())
    }

    async fn get_token_pair_prices(
        &self,
        amount: f64,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        let anchor_token = &self.anchor_token();
        let dexes = &self.dexes();
        let tokens = &self.tokens();

        // Get prices with base token / each token and each token / base token
        let mut get_price_futures = Vec::new();
        for dex in dexes.iter() {
            let mut dex_get_price_futures = self
                .base_trader
                .get_token_pair_prices(&dex, anchor_token, tokens, amount)
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

    fn initial_amount(&self) -> f64 {
        self.base_trader.initial_amount()
    }

    fn tokens(&self) -> Arc<Vec<Box<dyn Token>>> {
        self.base_trader.tokens()
    }

    fn anchor_token(&self) -> Arc<Box<dyn Token>> {
        self.base_trader.anchor_token()
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
