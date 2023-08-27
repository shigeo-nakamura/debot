// fund_manager.rs

use crate::db::CounterType;
use crate::trade::trade_position::State;

use super::abstract_trader::ReasonForSell;
use super::price_history::MarketStatus;
use super::trade_position::TakeProfitStrategy;
use super::{DBHandler, DexPrices, PriceHistory, TradePosition, TradingStrategy};
use std::collections::HashMap;
use std::f64;
use std::sync::Arc;
use tokio::sync::Mutex;

const MOVING_AVERAGE_WINDOW_SIZE: usize = 5;

#[derive(PartialEq)]
enum FundState {
    Active,
    ShouldLiquidate,
    Liquidated,
}

pub struct FundManagerState {
    amount: f64,
    open_positions: HashMap<String, TradePosition>,
    fund_state: Arc<std::sync::Mutex<FundState>>,
    score: f64,
    past_scores: Vec<f64>,
    db_handler: Arc<Mutex<DBHandler>>,
}

pub struct FundManagerConfig {
    name: String,
    token_name: String,
    strategy: TradingStrategy,
    trade_period: usize,
    buy_signal_threshold: f64,
    max_hold_interval_in_secs: u64,
    risk_reward: f64,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
}

pub struct TradeProposal {
    pub profit: f64,
    pub predicted_price: Option<f64>,
    pub execution_price: f64,
    pub amount: f64,
    pub fund_name: String,
    pub reason_for_sell: Option<ReasonForSell>,
    pub atr: Option<f64>,
    pub market_status: Option<MarketStatus>,
}

impl FundManager {
    pub fn new(
        fund_name: &str,
        token_name: &str,
        open_positions: Option<HashMap<String, TradePosition>>,
        strategy: TradingStrategy,
        trade_period: usize,
        initial_amount: f64,
        initial_score: f64,
        buy_signal_threshold: f64,
        max_hold_interval_in_secs: u64,
        risk_reward: f64,
        db_handler: Arc<Mutex<DBHandler>>,
    ) -> Self {
        let config = FundManagerConfig {
            name: fund_name.to_owned(),
            token_name: token_name.to_owned(),
            strategy,
            trade_period,
            buy_signal_threshold,
            max_hold_interval_in_secs,
            risk_reward,
        };

        let open_positions = match open_positions {
            Some(positions) => positions,
            None => HashMap::new(),
        };

        let state = FundManagerState {
            amount: initial_amount,
            open_positions,
            fund_state: Arc::new(std::sync::Mutex::new(FundState::Active)),
            score: initial_score,
            past_scores: vec![],
            db_handler,
        };

        Self { config, state }
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub fn score(&self) -> f64 {
        self.state.score
    }

    pub fn amount(&self) -> f64 {
        self.state.amount
    }

    pub fn set_amount(&mut self, amount: f64) {
        log::info!("{}'s new amount = {:6.5}", self.name(), amount);
        self.state.amount = amount;
    }

    fn amount_of_positinos_in_base_token(&self) -> f64 {
        let mut amount = 0.0;
        for position in self.state.open_positions.values() {
            amount += position.amount_in_base_token;
        }
        amount
    }

    pub fn begin_liquidate(&self) {
        let mut fund_state = self.state.fund_state.lock().unwrap();
        *fund_state = FundState::ShouldLiquidate;
    }

    pub fn end_liquidate(&self) {
        let mut fund_state = self.state.fund_state.lock().unwrap();
        if *fund_state == FundState::ShouldLiquidate {
            *fund_state = FundState::Liquidated;
        }
    }

    pub fn is_liquidated(&self) -> bool {
        *self.state.fund_state.lock().unwrap() == FundState::Liquidated
    }

    pub fn find_buy_opportunities(
        &self,
        token_name: &str,
        buy_price: f64,
        sell_price: f64,
        spread: f64,
        amount: f64,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Option<TradeProposal> {
        if token_name != self.config.token_name {
            return None;
        }

        if let Some(history) = histories.get_mut(token_name) {
            // update ATR
            history.update_atr(self.config.trade_period);

            let predicted_price =
                history.majority_vote_predictions(self.config.trade_period, self.config.strategy);

            let price_spread = (buy_price - sell_price) / buy_price;
            let profit_ratio = (predicted_price - sell_price) / sell_price;

            let color = match profit_ratio {
                x if x > 0.0 => "\x1b[0;32m",
                x if x < 0.0 => "\x1b[0;31m",
                _ => "\x1b[0;90m",
            };
            log::debug!(
                "{} {:>7.3}%\x1b[0m({:>2.2}%), {:<30} \x1b[0;34m{:<6}\x1b[0m {:<6.5}({:<6.5}) - {:<6.5}",
                color,
                profit_ratio * 100.0,
                price_spread * 100.0,
                self.name(),
                token_name,
                sell_price,
                predicted_price,
                buy_price,
            );

            if profit_ratio * 100.0 >= self.config.buy_signal_threshold {
                if !self.can_create_new_position(token_name) {
                    log::debug!(
                        "{} Need to wait for a while to create a new position",
                        self.name()
                    );
                    return None;
                }

                if self.state.amount < amount {
                    log::debug!(
                        "No enough fund left({}): remaining = {:6.3} < amount = {:6.3}, invested = {:6.3}",
                        self.name(),
                        self.state.amount,
                        amount,
                        self.amount_of_positinos_in_base_token(),
                    );
                    return None;
                }

                let atr = history.atr(self.config.trade_period);
                if atr.is_none() {
                    return None;
                }

                if atr.unwrap() < spread * sell_price {
                    log::info!("ATR: {:6.3} < {:6.3}", atr.unwrap(), spread * sell_price);
                    return None;
                }

                let profit = (predicted_price - buy_price) * amount;

                return Some(TradeProposal {
                    profit,
                    predicted_price: Some(predicted_price),
                    execution_price: buy_price,
                    amount,
                    fund_name: self.config.name.to_owned(),
                    reason_for_sell: None,
                    atr,
                    market_status: Some(history.market_status()),
                });
            }
        }
        None
    }

    pub fn check_positions(&self, current_prices: &HashMap<String, DexPrices>) {
        let mut unrealized_pnl = 0.0;

        for (token_name, prices) in current_prices {
            if let Some(position) = self.state.open_positions.get(token_name) {
                // caliculate unrialized PnL
                position.print_info(prices.sell.price);
                unrealized_pnl +=
                    prices.sell.price * position.amount - position.amount_in_base_token;
            }
        }

        if self.amount() + unrealized_pnl < 0.0 {
            log::info!(
                "This fund({}) should be liquidated. remaing_amount = {:6.3}, unrealized_pnl = {:6.3}",
                self.name(),
                self.amount(),
                unrealized_pnl
            );
            self.begin_liquidate();
        }
    }

    pub fn find_sell_opportunities(
        &self,
        token_name: &str,
        sell_price: f64,
        limitied_sell: bool,
    ) -> Option<TradeProposal> {
        if let Some(position) = self.state.open_positions.get(token_name) {
            let mut amount = 0.0;
            let mut reason_for_sell;

            let should_liquidate =
                *self.state.fund_state.lock().unwrap() == FundState::ShouldLiquidate;

            if should_liquidate {
                log::warn!(
                    "Close the position of {}, as its price{:.6} is requested to close",
                    token_name,
                    sell_price
                );
                reason_for_sell = Some(ReasonForSell::Liquidated);
            } else {
                reason_for_sell =
                    position.is_expired(self.config.max_hold_interval_in_secs.try_into().unwrap());
            }

            if limitied_sell == false && reason_for_sell.is_none() {
                reason_for_sell = position.should_close(sell_price);
            }

            if reason_for_sell.is_some() {
                amount = position.amount;
            }

            if amount > 0.0 {
                let profit = (sell_price - position.average_buy_price) * amount;
                return Some(TradeProposal {
                    profit,
                    predicted_price: None,
                    execution_price: sell_price,
                    amount,
                    fund_name: self.config.name.to_owned(),
                    reason_for_sell,
                    atr: None,
                    market_status: None,
                });
            }
        }
        None
    }

    fn can_create_new_position(&self, token_name: &str) -> bool {
        self.state.open_positions.get(token_name).is_none()
    }

    pub async fn update_position(
        &mut self,
        is_buy_trade: bool,
        reason_for_sell: Option<ReasonForSell>,
        token_name: &str,
        amount_in: f64,
        amount_out: f64,
        atr: Option<f64>,
        market_status: Option<MarketStatus>,
    ) {
        log::debug!(
            "update_position: amount_in = {:6.6}, amount_out = {:6.6}",
            amount_in,
            amount_out
        );
        if is_buy_trade {
            if atr.is_none() {
                log::info!("No ATR");
                return;
            }

            self.state.amount -= amount_in;

            let average_price = amount_in / amount_out;
            let cut_loss_price = average_price - atr.unwrap() * 2.0;
            let distance = (average_price - cut_loss_price) * self.config.risk_reward;
            let take_profit_price = average_price + distance;

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                // if there are already open positions for this token, update them
                position.add(
                    average_price,
                    take_profit_price,
                    cut_loss_price,
                    amount_out,
                    amount_in,
                );

                let position_cloned = position.clone();
                self.state
                    .db_handler
                    .lock()
                    .await
                    .log_position(&position_cloned)
                    .await;
            } else {
                // else, create a new position
                let mut position = TradePosition::new(
                    token_name,
                    self.name(),
                    TakeProfitStrategy::TrailingStop,
                    average_price,
                    take_profit_price,
                    cut_loss_price,
                    amount_out,
                    amount_in,
                    atr,
                    market_status,
                );
                position.id = self
                    .state
                    .db_handler
                    .lock()
                    .await
                    .increment_counter(CounterType::Position);
                log::info!("Open a new position: {:?}", position);

                let position_cloned = position.clone();
                self.state
                    .db_handler
                    .lock()
                    .await
                    .log_position(&position_cloned)
                    .await;
                self.state
                    .open_positions
                    .insert(token_name.to_owned(), position);
            }
        } else {
            self.state.amount += amount_out;

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                let sold_price = amount_out / amount_in;

                let new_state = match reason_for_sell.unwrap() {
                    ReasonForSell::Liquidated => State::Liquidated,
                    ReasonForSell::Expired => State::Expired,
                    ReasonForSell::TakeProfit => State::TakeProfit,
                    ReasonForSell::CutLoss => State::CutLoss,
                };

                position.del(sold_price, amount_in, new_state);

                let position_cloned = position.clone();
                let amount = position.amount;
                self.state
                    .db_handler
                    .lock()
                    .await
                    .log_position(&position_cloned)
                    .await;

                if amount > 0.0 {
                    panic!(
                        "partial selling is not supported. The remaing amount = {}",
                        amount
                    );
                }

                self.state.open_positions.remove(token_name); // If all of this token has been sold, remove it from the open positions
                log::debug!(
                    "Sold all of token: {}. Removed from open positions.",
                    token_name
                );
            }
        }
    }

    fn calculate_moving_average_score(&self, window: usize) -> f64 {
        let n = self.state.past_scores.len();
        if n < window {
            return self.state.score; // not enough data to calculate the moving average
        }
        let sum: f64 = self.state.past_scores[n - window..].iter().sum();
        sum / window as f64
    }

    pub fn apply_reward_or_penalty(&mut self, multiplier: f64) {
        self.state.score *= multiplier;
        self.state.past_scores.push(self.state.score);
        let moving_average_score = self.calculate_moving_average_score(MOVING_AVERAGE_WINDOW_SIZE);
        log::trace!(
            "{}'s new score = {:6.2}, moving average score = {:6.2}",
            self.config.name,
            self.state.score,
            moving_average_score
        );
    }
}
