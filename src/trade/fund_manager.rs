// fund_manager.rs

use shared_mongodb::{database, ClientHolder};

use super::{OpenPosition, PriceHistory, TradingStrategy, TransactionLog};
use crate::db::TransactionLogItem;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
const MOVING_AVERAGE_WINDOW_SIZE: usize = 5;

pub struct FundManagerState {
    amount: f64,
    open_positions: HashMap<String, OpenPosition>,
    close_all_position: bool,
    score: f64,
    past_scores: Vec<f64>,
    transaction_log: Arc<TransactionLog>,
}

pub struct FundManagerConfig {
    fund_name: String,
    strategy: TradingStrategy,
    trade_period: usize,
    leverage: f64,
    position_creation_inteval: u64,
    take_profit_threshold: f64,
    cut_loss_threshold: f64,
    max_hold_interval_in_secs: u64,
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
        max_hold_interval_in_days: f64,
        transaction_log: Arc<TransactionLog>,
    ) -> Self {
        let config = FundManagerConfig {
            fund_name: fund_name.to_owned(),
            strategy,
            trade_period,
            leverage,
            position_creation_inteval,
            take_profit_threshold,
            cut_loss_threshold,
            max_hold_interval_in_secs: (max_hold_interval_in_days * (SECONDS_PER_DAY as f64))
                as u64,
        };

        let state = FundManagerState {
            amount: initial_amount,
            open_positions: HashMap::new(),
            close_all_position: false,
            score: initial_score,
            past_scores: vec![],
            transaction_log,
        };

        Self { config, state }
    }

    pub fn fund_name(&self) -> &str {
        &self.config.fund_name
    }

    pub fn score(&self) -> f64 {
        self.state.score
    }

    pub fn amount(&self) -> f64 {
        self.state.amount
    }

    pub fn set_amount(&mut self, amount: f64) {
        log::trace!("{}'s new amount = {:6.5}", self.fund_name(), amount);
        self.state.amount = amount;
    }

    pub fn amount_of_positinos_in_base_token(&self) -> f64 {
        let mut amount = 0.0;
        for position in self.state.open_positions.values() {
            amount += position.amount_in_base_token;
        }
        amount
    }

    pub fn close_all_positions(&mut self) {
        self.state.close_all_position = true;
    }

    pub fn find_buy_opportunities(
        &self,
        token_name: &str,
        price: f64,
        histories: &mut HashMap<String, PriceHistory>,
    ) -> Option<TradeProposal> {
        if let Some(history) = histories.get_mut(token_name) {
            let predicted_price =
                history.majority_vote_predictions(self.config.trade_period, self.config.strategy);
            let profit_ratio = (predicted_price - price) * 100.0 / price;

            let color = match profit_ratio {
                x if x > 0.0 => "\x1b[0;32m",
                x if x < 0.0 => "\x1b[0;31m",
                _ => "\x1b[0;90m",
            };
            log::debug!(
                "{} {:3.3}%\x1b[0m {} \x1b[0;34m{:<6}\x1b[0m current: {:6.5}, predict: {:6.5}",
                color,
                profit_ratio,
                self.fund_name(),
                token_name,
                price,
                predicted_price,
            );

            if profit_ratio >= self.config.take_profit_threshold {
                if !self.can_create_new_position(token_name) {
                    log::debug!(
                        "{} Need to wait for a while to create a new position",
                        self.fund_name()
                    );
                    return None;
                }

                let amount = self.state.amount * self.config.leverage;

                if self.state.amount < amount {
                    log::debug!(
                        "No enough found left: {:6.5} < {:6.5}",
                        self.state.amount,
                        amount
                    )
                }

                if history.is_flash_crash() {
                    log::info!(
                        "Skip this buy trade as price of {} is crashed({:6.5} --> {:6.5})",
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

                    if holding_interval > self.config.max_hold_interval_in_secs.try_into().unwrap()
                    {
                        log::info!("Close the position as it reaches the limit of hold period");
                        amount = position.amount;
                    } else if position.do_take_profit(sell_price)
                        || position.do_cut_loss(sell_price)
                    {
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

    fn can_create_new_position(&self, token_name: &str) -> bool {
        if let Some(position) = self.state.open_positions.get(token_name) {
            let current_time = chrono::Utc::now().timestamp();
            let time_since_last_creation = current_time - position.open_time;

            return time_since_last_creation < (self.config.position_creation_inteval as i64);
        }
        true
    }

    pub async fn update_position(
        &mut self,
        is_buy_trade: bool,
        token_name: &str,
        amount_in: f64,
        amount_out: f64,
        db_client: &Arc<Mutex<ClientHolder>>,
    ) -> Option<f64> {
        if is_buy_trade {
            self.state.amount -= amount_in;

            let average_price = amount_in / amount_out;
            let take_profit_price = average_price * self.config.take_profit_threshold;
            let cut_loss_price = average_price * self.config.cut_loss_threshold;

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                // if there are already open positions for this token, update them
                position.add(
                    token_name,
                    average_price,
                    take_profit_price,
                    cut_loss_price,
                    amount_out,
                );

                let position_cloned = position.clone();
                self.update_transaction_log(db_client, &position_cloned)
                    .await;
            } else {
                // else, create a new position
                let position = OpenPosition::new(
                    0, // todo
                    token_name,
                    average_price,
                    take_profit_price,
                    cut_loss_price,
                    amount_out,
                    amount_in,
                );
                self.update_transaction_log(db_client, &position).await;
                self.state
                    .open_positions
                    .insert(token_name.to_owned(), position);
            }
        } else {
            self.state.amount += amount_out;

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                position.amount -= amount_in;

                let average_price = amount_in / amount_out;
                let pnl = (average_price - position.average_price) * amount_in;
                log::info!("PNL = {:6.2}", pnl);

                position.del(token_name, average_price, amount_in);

                let position_cloned = position.clone();
                let amount = position.amount;
                self.update_transaction_log(db_client, &position_cloned)
                    .await;

                if amount <= 0.0 {
                    self.state.open_positions.remove(token_name); // If all of this token has been sold, remove it from the open positions
                    log::debug!(
                        "Sold all of token: {}. Removed from open positions.",
                        token_name
                    );
                }
                return Some(pnl);
            }
        }
        None
    }

    async fn update_transaction_log(
        &self,
        db_client: &Arc<Mutex<ClientHolder>>,
        position: &OpenPosition,
    ) {
        log::debug!("update_transaction_log");

        let mut item = TransactionLogItem::default();

        item.id = self.state.transaction_log.increment_counter();
        item.open_time = String::new(); //
        item.close_time = String::new(); //
        item.fund_name = self.fund_name().to_owned();
        item.event_type = String::new(); //
        item.token = String::new(); //
        item.buy_price = position.average_price;
        item.predicted_price = position.take_profit_price;
        item.sell_price = 0.0; //
        item.amount = position.amount;
        item.realized_pnl = 0.0; //

        let db = match database::get(db_client, self.state.transaction_log.db_name()).await {
            Ok(db) => db,
            Err(e) => {
                log::info!("{:?}", e);
                return;
            }
        };

        if let Err(e) = TransactionLog::update(&db, &item).await {
            log::info!("{:?}", e);
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
            self.config.fund_name,
            self.state.score,
            moving_average_score
        );
    }
}
