// fund_manager.rs

use crate::db::CounterType;

use super::DBHandler;
use debot_market_analyzer::{MarketData, TradeAction, TradeChance, TradingStrategy};
use debot_position_manager::{ReasonForClose, TakeProfitStrategy, TradePosition};
use dex_client::DexClient;
use std::collections::HashMap;
use std::error::Error;
use std::f64;
use std::sync::Arc;

use tokio::sync::Mutex;

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
    trading_period: usize,
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
        open_positions: Option<HashMap<String, TradePosition>>,
        market_data: MarketData,
        strategy: TradingStrategy,
        trading_period: usize,
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
            trading_period,
            dry_run,
            save_prices,
        };

        let open_positions = match open_positions {
            Some(positions) => positions,
            None => HashMap::new(),
        };

        let state = FundManagerState {
            amount: initial_amount,
            open_positions,
            fund_state: Arc::new(std::sync::Mutex::new(FundState::Active)),
            db_handler,
            dex_client,
            market_data,
        };

        Self { config, state }
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub fn amount(&self) -> f64 {
        self.state.amount
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

    pub async fn find_chances(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = &mut self.state.market_data;
        let token_name = &self.config.token_name;

        // Get the token price
        let result = self
            .state
            .dex_client
            .get_ticker(&self.config.token_name)
            .await
            .map_err(|_| format!("Failed to get the price of {}", token_name))?;
        let price = match result.price.parse::<f64>() {
            Ok(price) => price,
            Err(e) => return Err(Box::new(e)),
        };

        log::debug!("{}: {}", token_name, price);

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
        data.update_atr(self.config.trading_period);

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
        if self.state.amount == 0.0 {
            return Ok(());
        }

        let token_name = &self.config.token_name;
        let data = &self.state.market_data;

        let prediction = data.predict(self.config.trading_period, self.config.strategy);
        let price_ratio = (prediction.price - current_price) / current_price;

        let color = match prediction.confidence {
            x if x >= 1.0 => "\x1b[0;32m",
            x if x < 0.0 => "\x1b[0;31m",
            _ => "\x1b[0;90m",
        };
        log::info!(
            "{} {:>7.3}%\x1b[0m, {:<30} \x1b[0;34m{:<6}\x1b[0m {:<6.5}(--> {:<6.5})",
            color,
            price_ratio * 100.0,
            self.name(),
            token_name,
            current_price,
            prediction.price,
        );

        if prediction.confidence >= 1.0 {
            if self.state.amount < self.config.trading_amount {
                log::debug!(
                    "No enough fund left({}): remaining = {:6.3}",
                    self.name(),
                    self.state.amount,
                );
                return Ok(());
            }

            let action = if price_ratio > 0.0 {
                TradeAction::BuyOpen
            } else {
                TradeAction::SellOpen
            };

            if (self.config.strategy == TradingStrategy::TrendFollowingLong
                && action == TradeAction::SellOpen)
                || (self.config.strategy == TradingStrategy::TrendFollowingShort
                    && action == TradeAction::BuyOpen)
            {
                return Ok(());
            }

            self.execute_chances(
                current_price,
                TradeChance {
                    token_name: self.config.token_name.clone(),
                    predicted_price: Some(prediction.price),
                    amount: self.config.trading_amount,
                    atr: data.atr(self.config.trading_period),
                    momentum: Some(data.momentum()),
                    action,
                    confidence: prediction.confidence,
                },
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn find_close_chances(&mut self, current_price: f64) -> Result<(), ()> {
        let token_name = &self.config.token_name;

        if let Some(position) = self.state.open_positions.get(token_name) {
            let reason_for_close;

            let should_liquidate =
                *self.state.fund_state.lock().unwrap() == FundState::ShouldLiquidate;

            if should_liquidate {
                log::warn!(
                    "Liquidate the position({}: {}",
                    token_name,
                    current_price
                );
                self.end_liquidate();
                reason_for_close = Some(ReasonForClose::Liquidated);
            } else {
                reason_for_close = position.should_close(current_price, None);
            }

            if reason_for_close.is_some() {
                self.execute_chances(
                    current_price,
                    TradeChance {
                        token_name: self.config.token_name.clone(),
                        predicted_price: None,
                        amount: position.amount,
                        atr: None,
                        momentum: None,
                        action: TradeAction::SellClose,
                        confidence: 0.0,
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
        let size = chance.amount.to_string();
        let side = if chance.action.is_buy() {
            "BUY"
        } else {
            "SELL"
        };

        log::info!(
            "Execute: symbol = {}, size = {}, side = {}",
            symbol,
            size,
            side
        );

        let executed_price;

        if self.config.dry_run {
            executed_price = current_price;
        } else {
            // Execute the transaction
            let result = self
                .state
                .dex_client
                .create_order(symbol, &size, side)
                .await;
            if let Err(e) = result {
                log::error!("create_order failed: {:?}", e);
                return Err(());
            }
            let result = result.unwrap();
            if result.result == "Err" {
                log::error!("create_order failed: {:?}", result.message);
                return Err(());
            }
            executed_price = if result.price.is_none() {
                log::info!("The price executed is unknown");
                current_price
            } else {
                result.price.unwrap()
            };
        }

        let amount_in = chance.amount;
        let amount_out = amount_in / executed_price;

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
    ) {
        log::debug!(
            "update_position: amount_in = {:6.6}, amount_out = {:6.6}",
            amount_in,
            amount_out
        );

        if trade_action.is_open() {
            self.state.amount -= amount_in;

            let average_price = amount_in / amount_out;

            let cut_loss_price;
            let distance;
            let take_profit_price;
            if trade_action.is_buy() {
                cut_loss_price = average_price - atr.unwrap() / 2.0;
                distance = (average_price - cut_loss_price) * self.config.risk_reward;
                take_profit_price = average_price + distance;
            } else {
                cut_loss_price = average_price + atr.unwrap() / 2.0;
                distance = (cut_loss_price - average_price) * self.config.risk_reward;
                take_profit_price = average_price - distance;
            }

            if let Some(position) = self.state.open_positions.get_mut(token_name) {
                // if there are already open positions for this token, update them
                position.add(
                    average_price,
                    trade_action.is_buy(),
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
                    TakeProfitStrategy::FixedThreshold,
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
                let close_price = amount_out / amount_in;
                position.del(close_price, &reason_for_close.unwrap().to_string());

                let position_cloned = position.clone();
                let amount = position.amount;
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

                self.state.open_positions.remove(token_name);
            }
        }
    }

    pub fn check_positions(&self, price: f64) {
        let unrealized_pnl =
            if let Some(position) = self.state.open_positions.get(&self.config.token_name) {
                // caliculate unrialized PnL
                position.print_info(price);

                let current_val = if position.is_long_position {
                    price * position.amount
                } else {
                    -price * position.amount
                };

                current_val - position.amount_in_anchor_token
            } else {
                0.0
            };

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
}
