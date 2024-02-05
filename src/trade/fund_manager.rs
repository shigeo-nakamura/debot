// fund_manager.rs

use super::DBHandler;
use debot_market_analyzer::{MarketData, TradeAction, TradingStrategy};
use debot_position_manager::{ReasonForClose, State, TradePosition};
use dex_connector::{CreateOrderResponse, DexConnector, DexError, OrderSide};
use lazy_static::lazy_static;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::{env, f64};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct TradeChance {
    pub action: TradeAction,
    pub token_name: String,
    pub predicted_price: Option<f64>,
    pub amount: f64,
    pub atr: Option<f64>,
    pub position_id: Option<u32>,
}

pub struct FundManagerState {
    amount: f64,
    trade_positions: HashMap<u32, TradePosition>,
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<dyn DexConnector>,
    market_data: MarketData,
    last_trade_time: Option<i64>,
    dry_run_counter: usize,
}

pub struct FundManagerConfig {
    fund_name: String,
    index: usize,
    token_name: String,
    strategy: TradingStrategy,
    risk_reward: f64,
    trading_amount: f64,
    dry_run: bool,
    save_prices: bool,
    non_trading_period_secs: i64,
    order_effective_duration_secs: i64,
    use_market_order: bool,
    check_market_range: bool,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
}

const MIN_PRICE_CHANGE: f64 = 0.003; // 0.3%
const MAX_PRICE_CHANGE: f64 = 0.01; // 1.0%

const PRECISION_MULTIPLIER: f64 = 10000.0;

lazy_static! {
    static ref TAKE_PROFIT_RATIO: Option<f64> =
        env::var("TAKE_PROFIT_RATIO").map_or_else(|_| None, |v| v.parse::<f64>().ok());
}

impl FundManager {
    pub fn new(
        fund_name: &str,
        index: usize,
        token_name: &str,
        open_positions: Option<HashMap<u32, TradePosition>>,
        market_data: MarketData,
        strategy: TradingStrategy,
        trading_amount: f64,
        initial_amount: f64,
        risk_reward: f64,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_connector: Arc<dyn DexConnector>,
        dry_run: bool,
        save_prices: bool,
        non_trading_period_secs: i64,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        check_market_range: bool,
    ) -> Self {
        let config = FundManagerConfig {
            fund_name: fund_name.to_owned(),
            index,
            token_name: token_name.to_owned(),
            strategy,
            risk_reward,
            trading_amount,
            dry_run,
            save_prices,
            non_trading_period_secs,
            order_effective_duration_secs,
            use_market_order,
            check_market_range,
        };

        let open_positions = match open_positions {
            Some(positions) => positions,
            None => HashMap::new(),
        };

        let mut amount = initial_amount;

        let mut rng = rand::thread_rng();
        let dry_run_counter = rng.gen_range(1..=std::u16::MAX);
        for (_, position) in open_positions.clone() {
            amount -= position.amount_in_anchor_token();
        }
        log::info!("available amount = {}", amount);

        let state = FundManagerState {
            amount,
            trade_positions: open_positions,
            db_handler,
            dex_connector,
            market_data,
            last_trade_time: None,
            dry_run_counter: dry_run_counter.into(),
        };

        Self { config, state }
    }

    pub async fn initialize(&self, leverage: f64) {
        if self
            .state
            .dex_connector
            .set_leverage(self.token_name(), &leverage.to_string())
            .await
            .is_err()
        {
            panic!("Failed to set the leverage");
        }
    }

    pub fn fund_name(&self) -> &str {
        &self.config.fund_name
    }

    pub fn token_name(&self) -> &str {
        &self.config.token_name
    }

    pub async fn get_token_price(&mut self) -> Result<f64, Box<dyn Error + Send + Sync>> {
        let token_name = &self.config.token_name;

        // Get the token price
        let dex_connector = self.state.dex_connector.clone();

        // Get the token price
        let res = dex_connector
            .get_ticker(token_name)
            .await
            .map_err(|e| format!("Failed to get price of {}: {:?}", token_name, e).to_owned())?;
        log::trace!("{}: {:?}", token_name, res.price);

        Ok(res.price)
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

        if self.config.strategy != TradingStrategy::RangeGrid {
            self.find_expired_orders().await;
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

    async fn find_expired_orders(&mut self) {
        let positions_to_cancel: Vec<TradePosition> = self
            .state
            .trade_positions
            .iter()
            .filter(|(_k, v)| v.should_cancel_order())
            .map(|(_k, v)| v.clone())
            .collect();

        // Cancel the existing orders
        for position in &positions_to_cancel {
            self.cancel_order(position.order_id()).await;
        }
    }

    async fn find_open_chances(&mut self, current_price: f64) -> Result<(), ()> {
        let actions = self
            .state
            .market_data
            .is_open_signaled(self.config.strategy);

        self.handle_open_chances(current_price, &actions).await
    }

    async fn handle_open_chances(
        &mut self,
        current_price: f64,
        actions: &Vec<TradeAction>,
    ) -> Result<(), ()> {
        let token_name = self.config.token_name.to_owned();
        let data = self.state.market_data.to_owned();
        let mut updated_actions = actions.clone();

        if self.config.strategy == TradingStrategy::RangeGrid {
            if self.config.check_market_range && !data.is_range_bound().unwrap_or_default() {
                let _ = self.liquidate(Some(String::from("Out of range bound")));
                return Ok(());
            }

            // Cancel the orders that are out of range
            self.cancel_out_of_range_orders(&actions).await;

            // Ignore new duplicated orders
            updated_actions = self.ignore_duplicated_orders(actions);
        }

        let mut order_price = current_price;

        for action in updated_actions.clone() {
            let is_buy;
            let (mut target_price, confidence) = match action.clone() {
                TradeAction::BuyOpen(detail) => {
                    is_buy = true;
                    (detail.target_price(), detail.confidence())
                }
                TradeAction::SellOpen(detail) => {
                    is_buy = false;
                    (detail.target_price(), detail.confidence())
                }
                _ => continue,
            };

            if self.config.strategy == TradingStrategy::RangeGrid {
                order_price = target_price;
                target_price = if is_buy {
                    (1.0 + TAKE_PROFIT_RATIO.unwrap()) * order_price
                } else {
                    (1.0 - TAKE_PROFIT_RATIO.unwrap()) * order_price
                };
            } else {
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
                    "{}{:>7.3}%{}, {:<30} {}{:<6}{} {:<6.4}(--> {:<6.4})",
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

                if let Some(last_time) = self.state.last_trade_time {
                    if chrono::Utc::now().timestamp() - last_time
                        <= self.config.non_trading_period_secs
                    {
                        return Ok(());
                    }
                }

                target_price = match *TAKE_PROFIT_RATIO {
                    Some(v) => {
                        if is_buy {
                            current_price * (1.0 + v)
                        } else {
                            current_price * (1.0 - v)
                        }
                    }
                    None => {
                        if is_buy {
                            if price_ratio.abs() > MAX_PRICE_CHANGE {
                                current_price * (1.0 + MAX_PRICE_CHANGE)
                            } else {
                                target_price
                            }
                        } else {
                            if price_ratio.abs() > MAX_PRICE_CHANGE {
                                current_price * (1.0 - MAX_PRICE_CHANGE)
                            } else {
                                target_price
                            }
                        }
                    }
                };
            }

            if self.state.amount == 0.0 || self.state.amount < self.config.trading_amount {
                log::warn!("No enough fund left: {}", self.state.amount);
                return Ok(());
            }

            self.execute_chances(
                order_price,
                TradeChance {
                    token_name: self.config.token_name.clone(),
                    predicted_price: Some(target_price),
                    amount: self.config.trading_amount * confidence,
                    atr: Some(data.atr()),
                    action: action.clone(),
                    position_id: None,
                },
                None,
            )
            .await?;

            self.state.last_trade_time = Some(chrono::Utc::now().timestamp());
        }

        if self.config.strategy == TradingStrategy::RangeGrid {
            if updated_actions.len() > 0 {
                let mut positions_pairs: Vec<(&u32, &TradePosition)> =
                    self.state.trade_positions.iter().collect();
                positions_pairs.sort_by_key(|(_, v)| {
                    let price_as_fixed_point =
                        (v.ordered_price() * PRECISION_MULTIPLIER).round() as i64;
                    price_as_fixed_point
                });

                let adx = self.state.market_data.adx();

                for (_, position) in positions_pairs.iter().rev() {
                    if matches!(position.state(), State::Opening | State::Open) {
                        let side = if position.is_long_position() {
                            "Buy"
                        } else {
                            "Sell"
                        };
                        log::debug!(
                            "[{:<0.2}]{:>5}: {:<6.4}[{}]{}",
                            adx,
                            side,
                            position.ordered_price(),
                            position.order_id(),
                            if position.state() == State::Open {
                                "*"
                            } else {
                                ""
                            }
                        );
                    }
                }
            } else {
                log::trace!("{:<6.4}", current_price);
            }
        }

        Ok(())
    }

    async fn find_close_chances(&mut self, current_price: f64) -> Result<(), ()> {
        let cloned_open_positions = self.state.trade_positions.clone();

        for (position_id, position) in cloned_open_positions.iter() {
            if position.state() != State::Open {
                continue;
            }
            let action = self
                .state
                .market_data
                .is_close_signaled(self.config.strategy);

            self.handle_close_chances(current_price, *position_id, position, &action)
                .await?;
        }

        Ok(())
    }

    async fn handle_close_chances(
        &mut self,
        current_price: f64,
        position_id: u32,
        position: &TradePosition,
        action: &TradeAction,
    ) -> Result<(), ()> {
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
                    position_id: Some(position_id),
                },
                reason_for_close,
            )
            .await?;
        }

        Ok(())
    }

    async fn execute_chances(
        &mut self,
        order_price: f64,
        chance: TradeChance,
        reason_for_close: Option<ReasonForClose>,
    ) -> Result<(), ()> {
        let symbol = &self.config.token_name;
        let trade_amount = if chance.action.is_open() {
            chance.amount / order_price
        } else {
            chance.amount
        };
        let size = trade_amount.to_string();
        let side = if chance.action.is_buy() {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };
        let is_open = if chance.action.is_open() {
            "Open"
        } else {
            "Close"
        };

        log::info!(
            "Execute: {}, symbol = {}, order_price = {:<6.4?}, side = {:?}, reason = {:?}",
            is_open,
            symbol,
            order_price,
            side,
            reason_for_close
        );

        if self.config.dry_run {
            // Prepare a new/updated position
            self.state.dry_run_counter += 1;
            let order_id = self.state.dry_run_counter.to_string();
            self.prepare_position(
                &order_id,
                Some(order_price),
                chance.action,
                chance.predicted_price,
                reason_for_close,
                &chance.token_name,
                chance.atr,
                chance.position_id,
            )
            .await?;

            let mut rng = rand::thread_rng();
            if rng.gen::<f64>() < 0.5 {
                let filled_value = trade_amount * order_price;
                let fee = if self.config.strategy == TradingStrategy::RangeGrid {
                    0.0
                } else {
                    filled_value * 0.001
                };
                self.position_filled(order_id, filled_value, trade_amount, fee)
                    .await?;
            }
        } else {
            // Execute the transaction
            let order_price_str = if self.config.use_market_order {
                None
            } else {
                Some(order_price.to_string())
            };
            let res: Result<CreateOrderResponse, DexError> = self
                .state
                .dex_connector
                .create_order(symbol, &size, side.clone(), order_price_str)
                .await;
            match res {
                Ok(res) => {
                    let order_id = res.order_id;
                    // Prepare a new/updated position
                    log::info!("new order id = {}", order_id);
                    self.prepare_position(
                        &order_id,
                        Some(order_price),
                        chance.action,
                        chance.predicted_price,
                        reason_for_close,
                        &chance.token_name,
                        chance.atr,
                        chance.position_id,
                    )
                    .await?;
                }
                Err(e) => {
                    log::error!(
                        "create_order failed({}, {}, {:?}): {:?}",
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
        ordered_price: Option<f64>,
        trade_action: TradeAction,
        predicted_price: Option<f64>,
        reason_for_close: Option<ReasonForClose>,
        token_name: &str,
        atr: Option<f64>,
        position_id: Option<u32>,
    ) -> Result<(), ()> {
        let is_long_position = trade_action.is_buy();
        let position_cloned;

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
                return Err(());
            }

            let position = TradePosition::new(
                id.unwrap(),
                order_id,
                ordered_price.unwrap(),
                self.config.order_effective_duration_secs,
                token_name,
                &self.config.fund_name,
                is_long_position,
                predicted_price.unwrap(),
                atr,
            );

            position_cloned = position.clone();
            self.state
                .trade_positions
                .insert(position.id().unwrap_or_default(), position);
        } else {
            let position_id = position_id.unwrap();
            let position = self.state.trade_positions.get_mut(&position_id);
            if position.is_none() {
                log::warn!("The position not found: id = {}", position_id);
                return Err(());
            }
            let position = position.unwrap();
            position.close(order_id, &reason_for_close.unwrap().to_string())?;
            position_cloned = position.clone();
        }

        // Save the position in the DB
        self.state
            .db_handler
            .lock()
            .await
            .log_position(&position_cloned)
            .await;

        return Ok(());
    }

    fn find_position_from_order_id(&self, order_id: &str) -> Option<TradePosition> {
        match self
            .state
            .trade_positions
            .iter()
            .find(|(_id, position)| position.order_id() == order_id)
        {
            Some((_, position)) => Some(position.clone()),
            None => None,
        }
    }

    pub async fn position_filled(
        &mut self,
        order_id: String,
        filled_value: f64,
        filled_size: f64,
        fee: f64,
    ) -> Result<bool, ()> {
        if !self.config.dry_run {
            self.state
                .dex_connector
                .clear_filled_order(&self.config.token_name, &order_id)
                .await
                .map_err(|e| {
                    log::error!("{:?}", e);
                    ()
                })?;
        }

        let position = match self.find_position_from_order_id(&order_id) {
            Some(p) => p,
            None => {
                log::debug!(
                    "Filled position not found for {} in {}",
                    order_id,
                    self.config.fund_name
                );
                return Ok(false);
            }
        };

        let is_open_trade;
        let predicted_price = position.predicted_price();
        let is_long_position = position.is_long_position();

        let position_id = match position.id() {
            Some(p) => p,
            None => {
                log::error!("position id is None: order_id = {}", order_id);
                return Ok(false);
            }
        };

        is_open_trade = match position.state() {
            State::Opening => true,
            State::Closing(_) => false,
            _ => {
                log::warn!(
                    "This position is already filled, state: {:?}",
                    position.state()
                );
                return Ok(false);
            }
        };

        let price = filled_value / filled_size;
        let amount_in;
        let amount_out;
        if is_open_trade {
            amount_in = price * filled_size;
            amount_out = filled_size;
        } else {
            amount_in = filled_size;
            amount_out = price * filled_size;
        }

        log::info!(
            "fill_position: token_name = {}, order_id = {:?}, value = {:?}, size = {:?}, fee = {:?}",
            self.config.token_name,
            order_id,
            filled_value,
            filled_size,
            fee
        );

        let prev_amount = self.state.amount;
        let position_cloned;

        if is_open_trade {
            self.state.amount -= amount_in;
            let average_price = amount_in / amount_out;
            let take_profit_price = predicted_price;
            let distance = (take_profit_price - average_price).abs() / self.config.risk_reward;
            let cut_loss_price = if is_long_position {
                average_price - distance
            } else {
                average_price + distance
            };

            let position = self.state.trade_positions.get_mut(&position_id).unwrap();

            position.open(
                average_price,
                amount_out,
                amount_in,
                fee,
                take_profit_price,
                cut_loss_price,
            )?;
            position_cloned = position.clone();
            self.state.last_trade_time = Some(chrono::Utc::now().timestamp());
        } else {
            let position = self.state.trade_positions.get_mut(&position_id).unwrap();

            if is_long_position {
                self.state.amount += amount_out;
            } else {
                self.state.amount += position.amount_in_anchor_token() * 2.0 - amount_out;
            }

            let close_price = amount_out / amount_in;

            position.delete(Some(close_price), fee, false, None);
            position_cloned = position.clone();

            let amount = position.amount();
            if amount == 0.0 {
                self.state.trade_positions.remove(&position_id);
            } else {
                log::info!(
                    "Position is partially closed. The remaing amount = {}",
                    amount
                );
            }
        }

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

        return Ok(true);
    }

    async fn cancel_order(&mut self, order_id: &str) {
        let position = self
            .state
            .trade_positions
            .iter_mut()
            .find(|(_, pos)| pos.order_id() == order_id)
            .map(|(_, v)| v);

        let position = match position {
            Some(v) => v,
            None => return,
        };

        if !self.config.dry_run {
            if let Err(e) = self
                .state
                .dex_connector
                .cancel_order(position.order_id(), &self.config.token_name)
                .await
            {
                log::error!("{:?}", e);
                return;
            }
        }

        if !position.cancel() {
            log::warn!("Failed to cancel the order id = {}", order_id);
            return;
        }

        // Save the position in the DB
        self.state
            .db_handler
            .lock()
            .await
            .log_position(position)
            .await;

        self.state
            .trade_positions
            .retain(|_, position| position.order_id() != order_id);
    }

    fn ignore_duplicated_orders(&self, actions: &[TradeAction]) -> Vec<TradeAction> {
        let mut prices: HashSet<i64> = HashSet::new();
        self.state
            .trade_positions
            .iter()
            .filter(|(_, position)| matches!(position.state(), State::Open | State::Opening))
            .for_each(|(_, position)| {
                let fixed_point_price =
                    (position.ordered_price() * PRECISION_MULTIPLIER).round() as i64;
                prices.insert(fixed_point_price);
            });

        let new_actions: Vec<TradeAction> = actions
            .iter()
            .filter(|p| {
                p.target_price().is_some() && {
                    let target_price_fixed =
                        (p.target_price().unwrap() * PRECISION_MULTIPLIER).round() as i64;
                    !prices.contains(&target_price_fixed)
                }
            })
            .cloned()
            .collect();

        if !new_actions.is_empty() {
            log::debug!(
                "num of prices: {}, num of actions: {} --> {}",
                prices.len(),
                actions.len(),
                new_actions.len()
            );
            log::trace!("{:?}", prices);
        }

        new_actions
    }

    async fn cancel_out_of_range_orders(&mut self, actions: &[TradeAction]) {
        let (min_buy_price, max_buy_price, min_sell_price, max_sell_price) =
            match Self::find_min_max_trade_prices(actions) {
                Some(v) => v,
                None => {
                    log::error!("Price ranges are unkown");
                    return;
                }
            };

        let mut positions_to_cancel: Vec<TradePosition> = Vec::new();

        for (_, position) in &self.state.trade_positions {
            if position.state() != State::Opening {
                continue;
            }
            let price = position.ordered_price();
            if position.is_long_position() {
                if price < min_buy_price || max_buy_price < price {
                    positions_to_cancel.push(position.clone());
                }
            } else {
                if price < min_sell_price || max_sell_price < price {
                    positions_to_cancel.push(position.clone());
                }
            }
        }

        for position in &positions_to_cancel {
            log::debug!(
                "cancel {:<6.4}, order_id:{}, state:{:?}",
                position.ordered_price(),
                position.order_id(),
                position.state()
            );
            self.cancel_order(position.order_id()).await;
        }
    }

    fn find_min_max_trade_prices(actions: &[TradeAction]) -> Option<(f64, f64, f64, f64)> {
        let mut min_buy_price: Option<f64> = None;
        let mut max_buy_price: Option<f64> = None;
        let mut min_sell_price: Option<f64> = None;
        let mut max_sell_price: Option<f64> = None;

        for action in actions {
            match action {
                TradeAction::BuyOpen(detail) => {
                    let target_price = detail.target_price();
                    min_buy_price =
                        Some(min_buy_price.map_or(target_price, |min| min.min(target_price)));
                    max_buy_price =
                        Some(max_buy_price.map_or(target_price, |max| max.max(target_price)));
                }
                TradeAction::SellOpen(detail) => {
                    let target_price = detail.target_price();
                    min_sell_price =
                        Some(min_sell_price.map_or(target_price, |min| min.min(target_price)));
                    max_sell_price =
                        Some(max_sell_price.map_or(target_price, |max| max.max(target_price)));
                }
                _ => {}
            }
        }

        if min_buy_price.is_none()
            || max_buy_price.is_none()
            || min_sell_price.is_none()
            || max_sell_price.is_none()
        {
            return None;
        }

        Some((
            min_buy_price.unwrap(),
            max_buy_price.unwrap(),
            min_sell_price.unwrap(),
            max_sell_price.unwrap(),
        ))
    }

    pub async fn liquidate(&mut self, reason: Option<String>) {
        let res = self
            .state
            .dex_connector
            .cancel_all_orders(Some(self.config.token_name.clone()))
            .await;

        if let Err(e) = res {
            log::error!("liquidate failed (cancel): {:?}", e);
        }

        let res = self
            .state
            .dex_connector
            .close_all_positions(Some(self.config.token_name.clone()))
            .await;

        for (_, position) in self.state.trade_positions.iter_mut() {
            position.delete(None, 0.0, true, reason.clone());
            self.state
                .db_handler
                .lock()
                .await
                .log_position(&position)
                .await;
        }

        if let Err(e) = res {
            log::error!("liquidate failed (close): {:?}", e);
            return;
        }

        self.state.trade_positions.clear();
    }

    pub fn check_positions(&self, price: f64) {
        for (_, position) in &self.state.trade_positions {
            position.print_info(price);
            let _ = position.pnl(price);
        }
    }

    pub fn reset_dex_client(&mut self, dex_connector: Arc<dyn DexConnector>) {
        self.state.dex_connector = dex_connector;
    }
}
