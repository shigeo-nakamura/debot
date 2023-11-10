// fund_manager.rs

use crate::db::CounterType;

use super::DBHandler;
use debot_market_analyzer::{MarketData, TradeAction, TradingStrategy};
use debot_position_manager::{ReasonForClose, TradePosition};
use dex_client::DexClient;
use std::error::Error;
use std::f64;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
struct TradeChance {
    pub action: TradeAction,
    pub token_name: String,
    pub predicted_price: Option<f64>,
    pub amount: f64,
    pub atr: Option<f64>,
    pub momentum: Option<f64>,
    pub position_index: Option<usize>,
}

pub struct FundManagerState {
    amount: f64,
    balance: f64,
    open_positions: Vec<TradePosition>,
    db_handler: Arc<Mutex<DBHandler>>,
    dex_client: DexClient,
    market_data: MarketData,
}

pub struct FundManagerConfig {
    name: String,
    index: usize,
    token_name: String,
    strategy: TradingStrategy,
    risk_reward: f64,
    trading_amount: f64,
    initial_amount: f64,
    prediction_interval: usize,
    dry_run: bool,
    save_prices: bool,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
}

impl FundManager {
    pub fn new(
        fund_name: &str,
        index: usize,
        token_name: &str,
        open_positions: Option<Vec<TradePosition>>,
        market_data: MarketData,
        strategy: TradingStrategy,
        prediction_interval: usize,
        trading_amount: f64,
        initial_amount: f64,
        risk_reward: f64,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_client: DexClient,
        dry_run: bool,
        save_prices: bool,
    ) -> Self {
        let config = FundManagerConfig {
            name: fund_name.to_owned(),
            index,
            token_name: token_name.to_owned(),
            strategy,
            risk_reward,
            trading_amount,
            initial_amount,
            prediction_interval,
            dry_run,
            save_prices,
        };

        let open_positions = match open_positions {
            Some(positions) => {
                if strategy == TradingStrategy::TrendFollowReactive {
                    vec![]
                } else {
                    positions
                }
            }
            None => vec![],
        };

        let state = FundManagerState {
            balance: 0.0,
            amount: initial_amount,
            open_positions,
            db_handler,
            dex_client,
            market_data,
        };

        Self { config, state }
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub async fn find_chances(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = &mut self.state.market_data;
        let token_name = &self.config.token_name;

        // Get the token price
        let res = self
            .state
            .dex_client
            .get_ticker(&self.config.token_name)
            .await
            .map_err(|e| format!("Failed to get the price of {}. {:?}", token_name, e))?;

        let price = if res.result == "Err" {
            log::warn!("Price for {} is not available", token_name);
            None
        } else {
            match res.price.parse::<f64>() {
                Ok(price) => Some(price),
                Err(e) => return Err(Box::new(e)),
            }
        };

        log::debug!("{}: {:?}", token_name, price);

        // Update the market data and predict next prices
        let price_point = data.add_price(price, None);

        // Save the price in the DB
        if self.config.index == 0 && self.config.save_prices {
            self.state
                .db_handler
                .lock()
                .await
                .log_price(data.name(), token_name, price_point)
                .await;
        }

        // update ATR
        data.update_atr(self.config.prediction_interval);

        if price.is_none() {
            return Ok(());
        }
        let price = price.unwrap();

        self.find_open_chances(price)
            .await
            .map_err(|_| "Failed to find open chances".to_owned())?;

        self.find_close_chances(price)
            .await
            .map_err(|_| "Failed to find close chances".to_owned())?;

        self.check_positions(price);

        Ok(())
    }

    async fn find_open_chances(&mut self, current_price: f64) -> Result<(), ()> {
        let token_name = &self.config.token_name;
        let data = &self.state.market_data;

        let prediction = data.predict(self.config.prediction_interval, self.config.strategy);
        let price_ratio = (prediction.price - current_price) / current_price;

        let color = match prediction.confidence {
            x if x >= 0.5 && price_ratio > 0.0 => "\x1b[0;32m",
            x if x >= 0.5 && price_ratio < 0.0 => "\x1b[0;31m",
            _ => "\x1b[0;90m",
        };

        let log_message = format!(
            "{} {:>7.3}%\x1b[0m, {:<30} \x1b[0;34m{:<6}\x1b[0m {:<6.5}(--> {:<6.5})",
            color,
            price_ratio * 100.0,
            self.name(),
            token_name,
            current_price,
            prediction.price
        );
        if prediction.confidence == 0.0 {
            log::debug!("{}", log_message);
        } else {
            log::info!("{}", log_message);
        }

        if prediction.confidence >= 0.5 {
            if self.config.strategy != TradingStrategy::TrendFollowReactive {
                if self.state.amount < self.config.trading_amount {
                    log::debug!(
                        "No enough fund left({}): remaining = {:6.3}",
                        self.name(),
                        self.state.amount,
                    );
                    return Ok(());
                }
            }

            let predicted_price;
            let fee_percentage = 0.1;
            let action = if price_ratio > 0.0 {
                predicted_price = prediction.price * (1.0 + fee_percentage / 100.0);
                TradeAction::BuyOpen
            } else {
                predicted_price = prediction.price * (1.0 - fee_percentage / 100.0);
                TradeAction::SellOpen
            };

            if self.config.strategy == TradingStrategy::TrendFollowReactive {
                if (self.state.balance > self.config.initial_amount
                    && action == TradeAction::BuyOpen)
                    || (self.state.balance < -self.config.initial_amount
                        && action == TradeAction::SellOpen)
                {
                    log::debug!(
                        "No margine left({}): balance = {:6.3}",
                        self.name(),
                        self.state.balance
                    );
                    return Ok(());
                }
            } else {
                if (self.config.strategy == TradingStrategy::TrendFollowLong
                    && action == TradeAction::SellOpen)
                    || (self.config.strategy == TradingStrategy::TrendFollowShort
                        && action == TradeAction::BuyOpen)
                {
                    return Ok(());
                }
            }

            self.execute_chances(
                current_price,
                TradeChance {
                    token_name: self.config.token_name.clone(),
                    predicted_price: Some(predicted_price),
                    amount: self.config.trading_amount * prediction.confidence,
                    atr: data.atr(self.config.prediction_interval),
                    momentum: Some(data.momentum()),
                    action,
                    position_index: None,
                },
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn find_close_chances(&mut self, current_price: f64) -> Result<(), ()> {
        if self.config.strategy == TradingStrategy::TrendFollowReactive {
            return Ok(());
        }

        let cloned_open_positions = self.state.open_positions.clone();

        for position in cloned_open_positions {
            let reason_for_close = position.should_close(current_price);

            if reason_for_close.is_some() {
                self.execute_chances(
                    current_price,
                    TradeChance {
                        token_name: self.config.token_name.clone(),
                        predicted_price: None,
                        amount: position.amount(),
                        atr: None,
                        momentum: None,
                        action: if position.is_long_position() {
                            TradeAction::SellClose
                        } else {
                            TradeAction::BuyClose
                        },
                        position_index: Some(0),
                    },
                    reason_for_close,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn execute_chances(
        &mut self,
        current_price: f64,
        chance: TradeChance,
        reason_for_close: Option<ReasonForClose>,
    ) -> Result<(), ()> {
        let symbol = &self.config.token_name;
        let trade_amount = if chance.action.is_open() {
            chance.amount / current_price
        } else {
            chance.amount
        };
        let size = trade_amount.to_string();
        let side = if chance.action.is_buy() {
            "BUY"
        } else {
            "SELL"
        };

        log::debug!(
            "Execute: symbol = {}, size = {}, side = {}",
            symbol,
            size,
            side
        );

        let mut amount_in = trade_amount;
        let mut amount_out = if chance.action.is_open() {
            amount_in / current_price
        } else {
            amount_in * current_price
        };

        if !self.config.dry_run {
            // Execute the transaction
            let res = self
                .state
                .dex_client
                .create_order(symbol, &size, side)
                .await;
            if let Err(e) = res {
                log::error!("create_order failed({}, {}): {:?}", size, side, e);
                return Err(());
            }
            let result = res.unwrap();
            if result.result == "Err" {
                log::error!("create_order failed: {:?}", result.message);
                return Err(());
            }

            let executed_price = match result.price.parse::<f64>() {
                Ok(price) => price,
                Err(e) => {
                    log::error!("Failed to get the price executed: {:?}", e);
                    current_price
                }
            };
            match size.parse::<f64>() {
                Ok(size) => {
                    if chance.action.is_open() {
                        amount_in = executed_price * size;
                        amount_out = size;
                    } else {
                        amount_in = size;
                        amount_out = executed_price * size;
                    }
                }
                Err(e) => {
                    log::error!("Failed to get the size executed: {:?}", e);
                }
            }
        }

        // Update the position
        self.update_position(
            chance.action,
            reason_for_close,
            &chance.token_name,
            amount_in,
            amount_out,
            chance.atr,
            chance.momentum,
            chance.predicted_price,
            chance.position_index,
        )
        .await;

        Ok(())
    }

    pub async fn update_position(
        &mut self,
        trade_action: TradeAction,
        reason_for_close: Option<ReasonForClose>,
        token_name: &str,
        amount_in: f64,
        amount_out: f64,
        atr: Option<f64>,
        momentum: Option<f64>,
        predicted_price: Option<f64>,
        position_index: Option<usize>,
    ) {
        log::debug!(
            "update_position: amount_in = {:6.6}, amount_out = {:6.6}",
            amount_in,
            amount_out
        );

        let prev_amount = self.state.amount;
        let prev_balance = self.state.balance;

        if trade_action.is_open() {
            if self.config.strategy == TradingStrategy::TrendFollowReactive {
                if trade_action.is_buy() {
                    self.state.balance += amount_in;
                } else {
                    self.state.balance -= amount_in;
                }
            } else {
                self.state.amount -= amount_in;
            }

            let average_price = amount_in / amount_out;

            let take_profit_price = predicted_price.unwrap();
            let distance = (take_profit_price - average_price).abs() / self.config.risk_reward;
            let cut_loss_price = if trade_action.is_buy() {
                average_price - distance
            } else {
                average_price + distance
            };

            // create a new position
            let mut position = TradePosition::new(
                token_name,
                self.name(),
                average_price,
                trade_action.is_buy(),
                take_profit_price,
                cut_loss_price,
                amount_out,
                amount_in,
                atr,
                momentum,
                predicted_price,
            );
            position.set_id(
                self.state
                    .db_handler
                    .lock()
                    .await
                    .increment_counter(CounterType::Position),
            );

            let position_cloned = position.clone();
            self.state
                .db_handler
                .lock()
                .await
                .log_position(&position_cloned)
                .await;
            self.state.open_positions.push(position);
        } else {
            let position_index = position_index.unwrap();
            let position = self.state.open_positions.get_mut(position_index);
            if position.is_none() {
                log::warn!("The position not found: index = {}", position_index);
                return;
            }
            let position = position.unwrap();

            if position.is_long_position() {
                self.state.amount += amount_out;
            } else {
                self.state.amount += position.amount_in_anchor_token() * 2.0 - amount_out;
            }

            let close_price = amount_out / amount_in;
            position.del(close_price, &reason_for_close.unwrap().to_string());

            let position_cloned = position.clone();
            let amount = position.amount();
            self.state
                .db_handler
                .lock()
                .await
                .log_position(&position_cloned)
                .await;

            if amount != 0.0 {
                log::error!(
                    "Position is partially closed. The remaing amount = {}",
                    amount
                );
            }

            self.state.open_positions.remove(position_index);
        }

        if self.config.strategy == TradingStrategy::TrendFollowReactive {
            log::info!(
                "{} Balance has changed from {} to {}",
                self.config.name,
                prev_balance,
                self.state.balance
            );
        } else {
            log::info!(
                "{} Amount has changed from {} to {}",
                self.config.name,
                prev_amount,
                self.state.amount
            );
        }
    }

    pub fn check_positions(&self, price: f64) {
        for position in &self.state.open_positions {
            position.print_info(price);
            let _ = position.pnl(price);
        }
    }

    pub fn reset_dex_client(&mut self, dex_client: DexClient) {
        self.state.dex_client = dex_client;
    }
}
