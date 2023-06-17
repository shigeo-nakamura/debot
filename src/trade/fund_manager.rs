// fund_manager.rs

use super::{OpenPosition, PriceHistory, TradingStrategy};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

pub struct FundManagerState {
    amount: f64,
    open_positions: HashMap<String, OpenPosition>,
    close_all_position: bool,
    score: f64,
    last_position_creation_time: SystemTime,
}

pub struct FundManagerConfig {
    fund_name: String,
    strategy: TradingStrategy,
    trade_period: usize,
    leverage: f64,
    position_creation_inteval: u64,
    take_profit_threshold: f64,
    cut_loss_threshold: f64,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
}

pub struct TradeProposal {
    pub profit: f64,
    pub price: f64,
    pub predicted_price: f64,
    pub amount: f64,
    pub fund_name: String,
}

impl FundManager {
    pub fn new(
        fund_name: &str,
        strategy: TradingStrategy,
        trade_period: usize,
        leverage: f64,
        initial_amount: f64,
        initial_score: f64,
        position_creation_inteval: u64,
        take_profit_threshold: f64,
        cut_loss_threshold: f64,
    ) -> Self {
        let config = FundManagerConfig {
            fund_name: fund_name.to_owned(),
            strategy,
            trade_period,
            leverage,
            position_creation_inteval,
            take_profit_threshold,
            cut_loss_threshold,
        };

        let state = FundManagerState {
            amount: initial_amount,
            open_positions: HashMap::new(),
            close_all_position: false,
            score: initial_score,
            last_position_creation_time: SystemTime::now(),
        };

        Self { config, state }
    }

    pub fn fund_name(&self) -> &str {
        &self.config.fund_name
    }

    pub fn close_all_positions(&mut self) -> () {
        self.state.close_all_position = true;
    }

    pub fn find_buy_opportunities(
        &self,
        token_name: &str,
        price: f64,
        histories: &HashMap<String, PriceHistory>,
    ) -> Option<TradeProposal> {
        if let Some(history) = histories.get(token_name) {
            let predicted_price =
                history.majority_vote_predictions(self.config.trade_period, self.config.strategy);
            let profit_ratio = (predicted_price - price) / price;

            log::debug!(
                "{:<6}: #{:6.3}%, current: {:6.3}, predict: {:6.3}",
                token_name,
                profit_ratio,
                price,
                predicted_price,
            );

            let amount = self.state.amount * self.config.leverage;

            if self.state.amount < amount {
                log::debug!(
                    "No enough found left: {:6.3} < {:6.3}",
                    self.state.amount,
                    amount
                )
            }

            if profit_ratio > self.config.take_profit_threshold {
                if history.is_flash_crash() {
                    log::info!(
                        "Skip this buy trade as price of {} is crashed({:6.3} --> {:6.3})",
                        token_name,
                        price,
                        predicted_price
                    );
                    return None;
                }

                let profit = (predicted_price - price) * amount;

                return Some(TradeProposal {
                    profit,
                    price,
                    predicted_price,
                    amount,
                    fund_name: self.config.fund_name.to_owned(),
                });
            }
        }
        None
    }

    pub fn find_sell_opportunities(
        &self,
        token_name: &str,
        sell_price: f64,
        histories: &HashMap<String, PriceHistory>,
        max_hold_interval: u64,
    ) -> Option<TradeProposal> {
        if let Some(history) = histories.get(token_name) {
            if let Some(position) = self.state.open_positions.get(token_name) {
                position.print_info(token_name, sell_price);

                let mut amount = 0.0;

                if history.is_flash_crash() || self.state.close_all_position {
                    log::info!(
                        "Close the position of {}, as its price{:.6} is crashed, or requested to close",
                        token_name,
                        sell_price
                    );
                    amount = position.amount;
                } else {
                    let current_time = chrono::Utc::now().timestamp();
                    let holding_interval = current_time - position.open_time;

                    if holding_interval > max_hold_interval.try_into().unwrap() {
                        log::info!("Close the position as it reaches the limit of hold period");
                        amount = position.amount;
                    } else if position.do_take_profit() || position.do_cut_loss() {
                        amount = position.amount;
                    }
                }

                if amount > 0.0 {
                    let profit = (sell_price - position.average_price) * amount;
                    return Some(TradeProposal {
                        profit,
                        price: sell_price,
                        predicted_price: sell_price,
                        amount,
                        fund_name: self.config.fund_name.to_owned(),
                    });
                }
            }
        }
        None
    }

    pub fn can_create_new_position(&self) -> bool {
        let current_time = SystemTime::now();
        let time_since_last_creation = current_time
            .duration_since(self.state.last_position_creation_time)
            .unwrap();

        time_since_last_creation < Duration::from_secs(self.config.position_creation_inteval)
    }

    pub fn update_position(
        &mut self,
        is_buy_trade: bool,
        token_name: &str,
        amount_in: f64,
        amount_out: f64,
    ) -> Option<f64> {
        // Update the last position creation time to the current time
        self.state.last_position_creation_time = SystemTime::now();

        if is_buy_trade {
            self.state.amount -= amount_in;

            let average_price = amount_in / amount_out;
            let take_profit_price = average_price * self.config.take_profit_threshold;
            let cut_loss_price = average_price * self.config.cut_loss_threshold;

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                // if there are already open positions for this token, update them
                position.update(
                    token_name,
                    average_price,
                    take_profit_price,
                    cut_loss_price,
                    amount_out,
                );
            } else {
                // else, create a new position
                let open_position = OpenPosition::new(
                    token_name,
                    average_price,
                    take_profit_price,
                    cut_loss_price,
                    amount_out,
                );
                self.state
                    .open_positions
                    .insert(token_name.to_owned(), open_position);
            }
        } else {
            self.state.amount += amount_out;

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                position.amount -= amount_in;

                let average_price = amount_in / amount_out;
                let pnl = (average_price - position.average_price) * amount_in;
                log::info!("PNL = {:6.2}", pnl);

                if position.amount <= 0.0 {
                    self.state.open_positions.remove(token_name); // If all of this token has been sold, remove it from the open positions
                    log::debug!(
                        "Sold all of token: {}. Removed from open positions.",
                        token_name
                    );
                } else {
                    log::debug!(
                        "Updated open position for token: {}, amount: {}, average price: {:.6}",
                        token_name,
                        position.amount,
                        position.average_price
                    );
                }
                return Some(pnl);
            }
        }
        None
    }

    pub fn apply_reward_or_penalty(&mut self, multiplier: f64) -> () {
        self.state.score *= multiplier;
        log::trace!(
            "{}'s new score = {:6.2}",
            self.config.fund_name,
            self.state.score
        );
    }
}
