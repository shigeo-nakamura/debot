// fund_manager.rs

use super::DBHandler;
use debot_market_analyzer::{MarketData, TradeAction, TradingStrategy};
use debot_position_manager::{ReasonForClose, State, TradePosition};
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
    pub position_index: Option<usize>,
}

pub struct FundManagerState {
    amount: f64,
    open_positions: Vec<TradePosition>,
    db_handler: Arc<Mutex<DBHandler>>,
    dex_client: DexClient,
    market_data: MarketData,
    last_trade_time: Option<i64>,
    dry_run_counter: usize,
}

pub struct FundManagerConfig {
    fund_name: String,
    dex_name: String,
    index: usize,
    token_name: String,
    strategy: TradingStrategy,
    risk_reward: f64,
    trading_amount: f64,
    dry_run: bool,
    save_prices: bool,
    non_trading_period_secs: i64,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
}

const MIN_PRICE_CHANGE: f64 = 0.005; // 0.5%
const MAX_PRICE_CHANGE: f64 = 0.015; // 1.5%

impl FundManager {
    pub fn new(
        fund_name: &str,
        dex_name: &str,
        index: usize,
        token_name: &str,
        open_positions: Option<Vec<TradePosition>>,
        market_data: MarketData,
        strategy: TradingStrategy,
        trading_amount: f64,
        initial_amount: f64,
        risk_reward: f64,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_client: DexClient,
        dry_run: bool,
        save_prices: bool,
        non_trading_period_secs: i64,
    ) -> Self {
        let config = FundManagerConfig {
            fund_name: fund_name.to_owned(),
            dex_name: dex_name.to_owned(),
            index,
            token_name: token_name.to_owned(),
            strategy,
            risk_reward,
            trading_amount,
            dry_run,
            save_prices,
            non_trading_period_secs,
        };

        let open_positions = match open_positions {
            Some(positions) => positions,
            None => vec![],
        };

        let mut amount = initial_amount;
        let dry_run_counter = open_positions.len();
        for position in open_positions.clone() {
            amount -= position.amount_in_anchor_token();
        }
        log::info!("available amount = {}", amount);

        let state = FundManagerState {
            amount,
            open_positions,
            db_handler,
            dex_client,
            market_data,
            last_trade_time: None,
            dry_run_counter,
        };

        Self { config, state }
    }

    pub fn fund_name(&self) -> &str {
        &self.config.fund_name
    }

    pub fn token_name(&self) -> &str {
        &self.config.token_name
    }

    pub async fn get_token_price(&mut self) -> Option<f64> {
        let token_name = &self.config.token_name;

        // Get the token price
        let res = self
            .state
            .dex_client
            .get_ticker(&self.config.dex_name, token_name)
            .await;

        let res = match res {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to get price of {}: {:?}", token_name, e);
                return None;
            }
        };

        let price = match res.price {
            Some(price) => match price.parse::<f64>() {
                Ok(price) => Some(price),
                Err(e) => {
                    log::error!("{:?}", e);
                    return None;
                }
            },
            None => {
                log::warn!("Price is unknown");
                return None;
            }
        };

        log::trace!("{}: {:?}", token_name, price);

        price
    }

    pub async fn find_chances(&mut self, price: f64) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = &mut self.state.market_data;
        let price_point = data.add_price(Some(price), None);
        // Save the price in the DB
        if self.config.index == 0 && self.config.save_prices {
            self.state
                .db_handler
                .lock()
                .await
                .log_price(data.name(), &self.config.token_name, price_point)
                .await;
        }

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

        let action = data.is_open_signaled(self.config.strategy);

        let (target_price, confidence) = match action.clone() {
            TradeAction::None => {
                return Ok(());
            }
            TradeAction::BuyOpen(detail) => (detail.target_price(), detail.confidence()),
            TradeAction::BuyClose => {
                return Ok(());
            }
            TradeAction::SellOpen(detail) => (detail.target_price(), detail.confidence()),
            TradeAction::SellClose => {
                return Ok(());
            }
        };

        let price_ratio = (target_price - current_price) / current_price;

        const GREEN: &str = "\x1b[0;32m";
        const RED: &str = "\x1b[0;31m";
        const GREY: &str = "\x1b[0;90m";
        const RESET: &str = "\x1b[0m";
        const BLUE: &str = "\x1b[0;34m";

        let color = match price_ratio {
            x if x > 0.0 => GREEN,
            x if x < 0.0 => RED,
            _ => GREY,
        };

        let log_message = format!(
            "{}{:>7.3}%{}, {:<30} {}{:<6}{} {:<6.5}(--> {:<6.5})",
            color,
            price_ratio * 100.0,
            RESET,
            self.config.fund_name,
            BLUE,
            token_name,
            RESET,
            current_price,
            target_price,
        );
        if color == GREY {
            log::debug!("{}", log_message);
        } else {
            log::info!("{}", log_message);
        }

        if price_ratio.abs() < MIN_PRICE_CHANGE {
            return Ok(());
        }

        let atr = data.atr();

        if let Some(last_time) = self.state.last_trade_time {
            if chrono::Utc::now().timestamp() - last_time <= self.config.non_trading_period_secs {
                return Ok(());
            }
        }

        if self.state.amount < self.config.trading_amount {
            log::warn!("No enough fund left: {}", self.state.amount);
            return Ok(());
        }

        let predicted_price = match action {
            TradeAction::BuyOpen(_) => {
                if price_ratio.abs() > MAX_PRICE_CHANGE {
                    current_price * (1.0 + MAX_PRICE_CHANGE)
                } else {
                    target_price
                }
            }
            TradeAction::SellOpen(_) => {
                if price_ratio.abs() > MAX_PRICE_CHANGE {
                    current_price * (1.0 - MAX_PRICE_CHANGE)
                } else {
                    target_price
                }
            }
            _ => {
                return Ok(());
            }
        };

        self.state.last_trade_time = Some(chrono::Utc::now().timestamp());

        self.execute_chances(
            current_price,
            TradeChance {
                token_name: self.config.token_name.clone(),
                predicted_price: Some(predicted_price),
                amount: self.config.trading_amount * confidence,
                atr: Some(atr),
                action,
                position_index: None,
            },
            None,
        )
        .await?;
        Ok(())
    }

    async fn find_close_chances(&mut self, current_price: f64) -> Result<(), ()> {
        let cloned_open_positions = self.state.open_positions.clone();

        for position in cloned_open_positions {
            let action = self
                .state
                .market_data
                .is_close_signaled(self.config.strategy);
            let reason_for_close = match action {
                TradeAction::BuyClose => {
                    if !position.is_long_position() {
                        Some(ReasonForClose::Other("TredeChanged".to_owned()))
                    } else {
                        None
                    }
                }
                TradeAction::SellClose => {
                    if position.is_long_position() {
                        Some(ReasonForClose::Other("TrendChanged".to_owned()))
                    } else {
                        None
                    }
                }
                _ => position.should_close(current_price),
            };

            if reason_for_close.is_some() {
                self.execute_chances(
                    current_price,
                    TradeChance {
                        token_name: self.config.token_name.clone(),
                        predicted_price: None,
                        amount: position.amount(),
                        atr: None,
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

        log::info!(
            "Execute: symbol = {}, size = {}, side = {}, reason = {:?}",
            symbol,
            size,
            side,
            reason_for_close
        );

        if self.config.dry_run {
            // Prepare a new/updated position
            self.state.dry_run_counter += 1;
            let order_id = self.state.dry_run_counter.to_string();
            self.prepare_position(
                &order_id,
                chance.action,
                reason_for_close,
                &chance.token_name,
                chance.predicted_price,
                chance.atr,
                chance.position_index,
            )
            .await;

            let filled_value = trade_amount * current_price;
            let fee = filled_value * 0.001;

            self.position_filled(
                Some(order_id),
                Some(filled_value.to_string()),
                Some(size),
                Some(fee.to_string()),
            )
            .await;
        } else {
            // Execute the transaction
            let res: Result<dex_client::CreateOrderResponse, dex_client::DexError> = self
                .state
                .dex_client
                .create_order(
                    &self.config.dex_name,
                    symbol,
                    &size,
                    side,
                    Some(current_price.to_string()),
                )
                .await;
            match res {
                Ok(res) => {
                    let order_id = res.ordier_id;
                    match order_id {
                        Some(id) => {
                            // Prepare a new/updated position
                            log::info!("new order id = {}", id);
                            self.prepare_position(
                                &id,
                                chance.action,
                                reason_for_close,
                                &chance.token_name,
                                chance.predicted_price,
                                chance.atr,
                                chance.position_index,
                            )
                            .await;
                        }
                        None => {
                            log::error!("order id is unknown");
                            return Err(());
                        }
                    }
                }
                Err(e) => {
                    log::error!(
                        "create_order failed({}, {}, {}): {:?}",
                        symbol,
                        size,
                        side,
                        e
                    );
                    return Err(());
                }
            }
        }

        Ok(())
    }

    async fn prepare_position(
        &mut self,
        order_id: &str,
        trade_action: TradeAction,
        reason_for_close: Option<ReasonForClose>,
        token_name: &str,
        predicted_price: Option<f64>,
        atr: Option<f64>,
        position_index: Option<usize>,
    ) {
        let is_long_position = trade_action.is_buy();

        if trade_action.is_open() {
            // create a new pending position
            let id = self
                .state
                .db_handler
                .lock()
                .await
                .increment_counter(debot_db::CounterType::Position);
            if id.is_none() {
                log::error!("Failed to increment the position ID");
                return;
            }

            let position = TradePosition::new(
                id.unwrap(),
                order_id,
                token_name,
                &self.config.fund_name,
                is_long_position,
                predicted_price.unwrap(),
                atr,
            );

            self.state.open_positions.push(position);
        } else {
            let position_index = position_index.unwrap();
            let position = self.state.open_positions.get_mut(position_index);
            if position.is_none() {
                log::warn!("The position not found: index = {}", position_index);
                return;
            }
            let position = position.unwrap();
            position.close(order_id, &reason_for_close.unwrap().to_string());
        }
    }

    pub async fn position_filled(
        &mut self,
        order_id: Option<String>,
        filled_value: Option<String>,
        filled_size: Option<String>,
        fee: Option<String>,
    ) -> bool {
        log::debug!(
            "fill_position: order_id = {:?}, value = {:?}, size = {:?}, fee = {:?}",
            order_id,
            filled_value,
            filled_size,
            fee
        );

        if order_id.is_none() || filled_value.is_none() || filled_size.is_none() || fee.is_none() {
            log::error!("filled order is wrong");
            return false;
        }

        let order_id = order_id.unwrap();
        let filled_value = filled_value.unwrap();
        let filled_size = filled_size.unwrap();
        let fee = fee.unwrap();

        let position_with_index = self
            .state
            .open_positions
            .iter_mut()
            .enumerate()
            .find(|(_index, pos)| pos.order_id() == order_id);

        let (position_index, position) = match position_with_index {
            Some((index, pos)) => (index, pos),
            None => {
                log::info!("Filled position not found for {}", order_id);
                return false;
            }
        };

        let amount_in;
        let amount_out;
        let is_open_trande = match position.state() {
            State::OpenPending => true,
            State::ClosePending(_) => false,
            _ => {
                log::info!(
                    "This position is already filled(expected), state: {:?}",
                    position.state()
                );
                return false;
            }
        };

        match filled_size.parse::<f64>() {
            Ok(size) => match filled_value.parse::<f64>() {
                Ok(value) => {
                    let price = value / size;
                    if is_open_trande {
                        amount_in = price * size;
                        amount_out = size;
                    } else {
                        amount_in = size;
                        amount_out = price * size;
                    }
                }
                Err(e) => {
                    log::error!("Failed to get the price executed: {:?}", e);
                    return false;
                }
            },
            Err(e) => {
                log::error!("Failed to get the size executed: {:?}", e);
                return false;
            }
        }

        let fee = fee.parse::<f64>().unwrap_or(0.0);

        let prev_amount = self.state.amount;

        let position_cloned;

        let is_open = match position.state() {
            State::OpenPending => true,
            State::ClosePending(_) => false,
            _ => {
                log::warn!("Filled position has an invalid state: {}", position.state());
                return false;
            }
        };

        if is_open {
            self.state.amount -= amount_in;
            let average_price = amount_in / amount_out;
            let take_profit_price = position.predicted_price();
            let distance = (take_profit_price - average_price).abs() / self.config.risk_reward;
            let cut_loss_price = if position.is_long_position() {
                average_price - distance
            } else {
                average_price + distance
            };

            position.open(
                average_price,
                amount_out,
                amount_in,
                fee,
                take_profit_price,
                cut_loss_price,
            );
            position_cloned = position.clone();
        } else {
            if position.is_long_position() {
                self.state.amount += amount_out;
            } else {
                self.state.amount += position.amount_in_anchor_token() * 2.0 - amount_out;
            }

            let close_price = amount_out / amount_in;

            position.delete(Some(close_price), fee, false, None);
            position_cloned = position.clone();

            let amount = position.amount();
            if amount == 0.0 {
                self.state.open_positions.remove(position_index);
            } else {
                log::info!(
                    "Position is partially closed. The remaing amount = {}",
                    amount
                );
            }
        }

        self.state.last_trade_time = Some(chrono::Utc::now().timestamp());

        // Save the position in the DB
        self.state
            .db_handler
            .lock()
            .await
            .log_position(&position_cloned)
            .await;

        log::info!(
            "{} Amount has changed from {} to {}",
            self.config.fund_name,
            prev_amount,
            self.state.amount
        );

        return true;
    }

    pub async fn liquidate(&mut self, reason: Option<String>) {
        let res = self
            .state
            .dex_client
            .close_all_positions(&self.config.dex_name, Some(self.config.token_name.clone()))
            .await;

        for position in self.state.open_positions.iter_mut() {
            position.delete(None, 0.0, true, reason.clone());
            self.state
                .db_handler
                .lock()
                .await
                .log_position(&position)
                .await;
        }

        if let Err(e) = res {
            log::error!("liquidate failed: {:?}", e);
            return;
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
