// fund_manager.rs

use super::dex_connector_box::DexConnectorBox;
use super::DBHandler;
use debot_market_analyzer::{MarketData, TradeAction, TradingStrategy};
use debot_position_manager::{PositionType, ReasonForClose, State, TradePosition};
use dex_connector::{CreateOrderResponse, DexConnector, DexError, OrderSide};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::f64;
use std::sync::Arc;
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

struct FundManagerState {
    amount: f64,
    trade_positions: HashMap<u32, TradePosition>,
    latest_open_position_id: Option<u32>,
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    market_data: MarketData,
    last_losscut_time: Option<i64>,
}

struct FundManagerConfig {
    fund_name: String,
    index: usize,
    token_name: String,
    strategy: TradingStrategy,
    risk_reward: f64,
    trading_amount: f64,
    initial_amount: f64,
    save_prices: bool,
    non_trading_period_secs: i64,
    order_effective_duration_secs: i64,
    use_market_order: bool,
    check_market_range: bool,
    grid_loss_cut_ratio: f64,
}

#[derive(Default)]
struct FundManagerStatics {
    order_count: u32,
    fill_count: u32,
    profit_count: u32,
    loss_count: u32,
    liquidate_count: u32,
    pnl: f64,
    min_amount: f64,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
    statistics: FundManagerStatics,
}

const MIN_PRICE_CHANGE: f64 = 0.003; // 0.3%
const MAX_PRICE_CHANGE: f64 = 0.01; // 1.0%

const PRECISION_MULTIPLIER: f64 = 10000.0;

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
        dex_connector: Arc<DexConnectorBox>,
        save_prices: bool,
        non_trading_period_secs: i64,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        check_market_range: bool,
        grid_loss_cut_ratio: f64,
    ) -> Self {
        let config = FundManagerConfig {
            fund_name: fund_name.to_owned(),
            index,
            token_name: token_name.to_owned(),
            strategy,
            risk_reward,
            trading_amount,
            initial_amount,
            save_prices,
            non_trading_period_secs,
            order_effective_duration_secs,
            use_market_order,
            check_market_range,
            grid_loss_cut_ratio,
        };

        let open_positions = match open_positions {
            Some(positions) => positions,
            None => HashMap::new(),
        };

        let mut amount = initial_amount;

        for (_, position) in open_positions.clone() {
            amount -= position.asset_in_usd().abs()
        }
        log::info!("available amount = {}", amount);

        let state = FundManagerState {
            amount,
            trade_positions: open_positions,
            db_handler,
            dex_connector,
            market_data,
            last_losscut_time: None,
            latest_open_position_id: None,
        };

        let mut statistics = FundManagerStatics::default();
        statistics.min_amount = initial_amount;

        Self {
            config,
            state,
            statistics,
        }
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

        self.find_expired_orders().await;

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
            if self.config.strategy == TradingStrategy::RangeGrid
                && !matches!(position.state(), State::Closing(_))
            {
                continue;
            }
            self.cancel_order(position.order_id()).await;
        }
    }

    async fn find_open_chances(&mut self, current_price: f64) -> Result<(), ()> {
        let (open_price, is_long) = match self.config.strategy {
            TradingStrategy::RangeGrid => {
                let open_position = self.get_open_position();
                match open_position {
                    Some(position) => (
                        Some(position.average_open_price()),
                        Some(position.position_type() == PositionType::Long),
                    ),
                    None => (None, None),
                }
            }
            _ => (None, None),
        };

        let actions =
            self.state
                .market_data
                .is_open_signaled(self.config.strategy, open_price, is_long);

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

        if let Some(last_time) = self.state.last_losscut_time {
            if chrono::Utc::now().timestamp() - last_time <= self.config.non_trading_period_secs {
                return Ok(());
            }
        }

        if self.config.strategy == TradingStrategy::RangeGrid {
            if self.config.check_market_range && !data.is_range_bound().unwrap_or_default() {
                let _ = self.liquidate(Some(String::from("Out of range bound")));
                return Ok(());
            }

            // Cancel the orders that are out of range
            self.cancel_out_of_range_orders(&actions, current_price)
                .await;

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
                    (1.0 + self.config.grid_loss_cut_ratio) * order_price
                } else {
                    (1.0 - self.config.grid_loss_cut_ratio) * order_price
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
                target_price = if is_buy {
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

            if self.state.amount < self.config.trading_amount {
                log::warn!("No enough fund left: {}", self.state.amount);
                self.liquidate(Some("NoEnoughFund".to_owned())).await;
            } else {
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
            }
        }

        if self.config.strategy == TradingStrategy::RangeGrid {
            // Cancel the orders that are out of range
            self.cancel_out_of_range_orders(&actions, current_price)
                .await;

            if updated_actions.len() > 0 {
                let mut positions_vec: Vec<TradePosition> = self
                    .state
                    .trade_positions
                    .iter()
                    .map(|(_, v)| v.clone())
                    .collect();
                positions_vec.sort_by_key(|v| {
                    let price_as_fixed_point =
                        (v.ordered_price() * PRECISION_MULTIPLIER).round() as i64;
                    price_as_fixed_point
                });

                let adx = self.state.market_data.adx();

                const GREEN: &str = "\x1b[0;32m";
                const RED: &str = "\x1b[0;31m";
                const RESET: &str = "\x1b[0m";
                const BLUE: &str = "\x1b[0;34m";

                for position in positions_vec.iter().rev() {
                    if matches!(
                        position.state(),
                        State::Opening | State::Open | State::Closing(_)
                    ) {
                        let (side, mut color) = match position.position_type() {
                            PositionType::Long => (format!("{:4}", "Buy"), BLUE),
                            PositionType::Short => (format!("{:4}", "Sell"), RED),
                        };
                        if position.state() == State::Open {
                            color = GREEN;
                        }

                        let is_updated = updated_actions.iter().any(|a| {
                            a.target_price().unwrap_or_default() == position.ordered_price()
                        });

                        log::debug!(
                            "[{:<0.2}] {:<5}: {}{:<6.4}{} [{}]({:.1}){}",
                            adx,
                            side,
                            color,
                            position.ordered_price(),
                            RESET,
                            position.order_id(),
                            position.asset_in_usd(),
                            match position.state() {
                                State::Open => "*",
                                State::Closing(_) => "-",
                                _ => {
                                    if is_updated {
                                        "+"
                                    } else {
                                        ""
                                    }
                                }
                            }
                        );
                    }
                }
                let max_price = match positions_vec.first() {
                    Some(p) => p.ordered_price(),
                    None => current_price,
                };
                let min_price = match positions_vec.last() {
                    Some(p) => p.ordered_price(),
                    None => current_price,
                };
                let spread = (max_price - min_price).abs();
                let spread_ratio = (spread / current_price) * 100.0;
                log::info!(
                    "pnl: {:.3}/{:.3}/{:.3} order/fill/liquidate/profit/loss = {}/{}/{}/{}/{}, min position = {:.1}, current = {:<6.4}, spread = {:<6.4}({:<1.3}%)",
                    self.statistics.pnl,
                    self.pnl_of_open_position(),
                    self.unrealized_pnl_of_open_position(current_price),
                    self.statistics.order_count,
                    self.statistics.fill_count,
                    self.statistics.liquidate_count,
                    self.statistics.profit_count,
                    self.statistics.loss_count,
                    self.statistics.min_amount,
                    current_price,
                    spread,
                    spread_ratio
                );
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
                if position.position_type() == PositionType::Short {
                    Some(ReasonForClose::Other("TredeChanged".to_owned()))
                } else {
                    None
                }
            }
            TradeAction::SellClose => {
                if position.position_type() == PositionType::Long {
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
                    action: if position.position_type() == PositionType::Long {
                        TradeAction::SellClose
                    } else {
                        TradeAction::BuyClose
                    },
                    position_id: Some(position_id),
                },
                reason_for_close,
            )
            .await?;

            self.state.last_losscut_time = Some(chrono::Utc::now().timestamp());
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
            OrderSide::Long
        } else {
            OrderSide::Short
        };
        let reason = match reason_for_close.clone() {
            Some(r) => r,
            None => ReasonForClose::Other(String::from("n/a")),
        };

        log::debug!(
            "Execute: {}, symbol = {}, order_price = {:<6.4?}, side = [{}], reason = {}",
            if chance.action.is_open() {
                "Open"
            } else {
                "Close"
            },
            symbol,
            order_price,
            side,
            reason,
        );

        // Execute the transaction
        let order_price_str = if chance.action.is_open() {
            if self.config.use_market_order {
                None
            } else {
                Some(order_price.to_string())
            }
        } else {
            None
        };

        let res: Result<CreateOrderResponse, DexError> = self
            .state
            .dex_connector
            .create_order(symbol, &size, side.clone(), order_price_str)
            .await;
        match res {
            Ok(res) => {
                let order_id = res.order_id;
                self.prepare_position(
                    &order_id,
                    Some(order_price),
                    trade_amount,
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

        Ok(())
    }

    async fn prepare_position(
        &mut self,
        order_id: &str,
        ordered_price: Option<f64>,
        ordered_amount: f64,
        trade_action: TradeAction,
        predicted_price: Option<f64>,
        reason_for_close: Option<ReasonForClose>,
        token_name: &str,
        atr: Option<f64>,
        position_id: Option<u32>,
    ) -> Result<(), ()> {
        let position_type = if trade_action.is_buy() {
            PositionType::Long
        } else {
            PositionType::Short
        };
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
                ordered_amount,
                self.config.order_effective_duration_secs,
                token_name,
                &self.config.fund_name,
                position_type,
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
            position.request_close(order_id, &reason_for_close.clone().unwrap().to_string())?;
            position_cloned = position.clone();

            match reason_for_close {
                Some(r) => match r {
                    ReasonForClose::TakeProfit => self.statistics.profit_count += 1,
                    ReasonForClose::CutLoss => self.statistics.loss_count += 1,
                    _ => {}
                },
                None => {}
            }
        }

        self.statistics.order_count += 1;

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

    fn get_open_position(&self) -> Option<TradePosition> {
        match self.state.latest_open_position_id {
            Some(id) => self.state.trade_positions.get(&id).cloned(),
            None => None,
        }
    }

    fn pnl_of_open_position(&self) -> f64 {
        match self.get_open_position() {
            Some(position) => position.pnl(),
            None => 0.0,
        }
    }

    fn unrealized_pnl_of_open_position(&self, price: f64) -> f64 {
        match self.get_open_position() {
            Some(position) => position.amount() * price + position.asset_in_usd(),
            None => 0.0,
        }
    }

    pub async fn position_filled(
        &mut self,
        order_id: String,
        filled_side: OrderSide,
        filled_value: f64,
        filled_size: f64,
        fee: f64,
    ) -> Result<bool, ()> {
        self.state
            .dex_connector
            .clear_filled_order(&self.config.token_name, &order_id)
            .await
            .map_err(|e| {
                log::error!("{:?}", e);
                ()
            })?;

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
        let position_type = position.position_type();

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
            "fill_position:{}, [{}], order_id = {:?}, value = {:?}, size = {:?}, fee = {:?}, price = {:<6.4}, amount_in = {}, amont_out = {}",
            self.config.token_name,
            filled_side,
            order_id,
            filled_value,
            filled_size,
            fee,
            price,
            amount_in,
            amount_out
        );

        let prev_amount = self.state.amount;
        let position_cloned;

        if is_open_trade {
            let average_price = amount_in / amount_out;
            let take_profit_price = predicted_price;
            let distance = (take_profit_price - average_price).abs() / self.config.risk_reward;
            let cut_loss_price = if filled_side == OrderSide::Long {
                Some(average_price - distance)
            } else {
                Some(average_price + distance)
            };
            let take_profit_price = if self.config.strategy == TradingStrategy::RangeGrid {
                None
            } else {
                Some(predicted_price)
            };

            let close_position;
            let open_position_id: Option<u32> =
                if self.config.strategy == TradingStrategy::RangeGrid {
                    self.state.latest_open_position_id
                } else {
                    None
                };
            let current_price = self.state.market_data.last_price();

            if let Some(open_position_id) = open_position_id {
                let open_position = match self.state.trade_positions.get_mut(&open_position_id) {
                    Some(v) => v,
                    None => {
                        log::error!("open position not found {}", open_position_id);
                        return Err(());
                    }
                };
                open_position.on_filled(
                    current_price,
                    position_type,
                    average_price,
                    amount_out,
                    amount_in,
                    fee,
                    take_profit_price,
                    cut_loss_price,
                )?;
                if matches!(open_position.state(), State::Closed(_)) {
                    self.state.latest_open_position_id = None;
                }
                let posstion_asset = open_position.asset_in_usd();
                self.state.amount = if posstion_asset > 0.0 {
                    self.config.initial_amount - posstion_asset
                } else {
                    self.config.initial_amount + posstion_asset
                };
                close_position = position.ordered_amount() == 0.0;
                position_cloned = open_position.clone();
            } else {
                self.state.amount -= amount_in;
                close_position = false;
                let position = self.state.trade_positions.get_mut(&position_id).unwrap();
                position.on_filled(
                    current_price,
                    position_type,
                    average_price,
                    amount_out,
                    amount_in,
                    fee,
                    take_profit_price,
                    cut_loss_price,
                )?;
                self.state.latest_open_position_id = position.id();
                position_cloned = position.clone();
            }
            if close_position {
                let position = self.state.trade_positions.get_mut(&position_id).unwrap();
                let is_closed = position.cancel()?;
                if is_closed {
                    self.state.trade_positions.remove(&position_id);
                } else {
                    log::error!("The position is not closed: {:?}", position);
                    return Err(());
                }
            }
        } else {
            let position = self.state.trade_positions.get_mut(&position_id).unwrap();
            if position_type == PositionType::Long {
                self.state.amount += amount_out;
            } else {
                self.state.amount += position.asset_in_usd() * 2.0 - amount_out;
            }

            let close_price = amount_out / amount_in;

            position.on_closed(Some(close_price), fee, false, None)?;
            position_cloned = position.clone();

            let amount = position.amount();
            if amount == 0.0 {
                self.state.trade_positions.remove(&position_id);
                self.state.latest_open_position_id = None;
                self.statistics.pnl += position_cloned.pnl();
            } else {
                log::info!(
                    "Position is partially closed. The remaing amount = {}",
                    amount
                );
            }
        }

        self.statistics.fill_count += 1;

        // Save the position in the DB
        self.state
            .db_handler
            .lock()
            .await
            .log_position(&position_cloned)
            .await;

        if self.state.amount < self.statistics.min_amount {
            self.statistics.min_amount = self.state.amount;
        }

        log::debug!(
            "{} Amount has changed from {:.1} to {:.1}",
            self.config.fund_name,
            prev_amount,
            self.state.amount
        );

        return Ok(true);
    }

    async fn cancel_order(&mut self, order_id: &str) {
        let mut position = match self.find_position_from_order_id(&order_id) {
            Some(v) => v,
            None => {
                log::error!("positino not found: order_id = {}", order_id);
                return;
            }
        };

        if let Err(e) = self
            .state
            .dex_connector
            .cancel_order(position.order_id(), &self.config.token_name)
            .await
        {
            log::error!("{:?}", e);
            return;
        }

        let is_closed = match position.cancel() {
            Ok(is_closed) => is_closed,
            Err(_) => {
                log::error!("Failed to cancel the position = {:?}", position);
                return;
            }
        };

        // Save the position in the DB
        self.state
            .db_handler
            .lock()
            .await
            .log_position(&position)
            .await;

        if is_closed {
            let position_id = match position.id() {
                Some(id) => id,
                None => {
                    log::error!("Position id is None: {:?}", position);
                    return;
                }
            };
            self.state.trade_positions.remove(&position_id);
        }
    }

    fn ignore_duplicated_orders(&self, actions: &[TradeAction]) -> Vec<TradeAction> {
        let mut prices: HashSet<i64> = HashSet::new();
        self.state
            .trade_positions
            .iter()
            .filter(|(_, position)| matches!(position.state(), State::Opening))
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

    async fn cancel_out_of_range_orders(&mut self, actions: &[TradeAction], current_price: f64) {
        let (buy_price, sell_price) = Self::find_min_max_trade_prices(actions);
        if buy_price.is_none() && sell_price.is_none() {
            log::warn!("Price ranges are unknown: {:?}", actions);
        }

        let mut positions_to_cancel: Vec<TradePosition> = Vec::new();

        let open_position = self.get_open_position();
        for (_, position) in &self.state.trade_positions {
            if position.state() != State::Opening {
                continue;
            }

            let price = position.ordered_price();
            if position.position_type() == PositionType::Long {
                if buy_price.is_some() {
                    if price < buy_price.unwrap().0 || buy_price.unwrap().1 < price {
                        positions_to_cancel.push(position.clone());
                        continue;
                    }
                }
            } else {
                if sell_price.is_some() {
                    if price < sell_price.unwrap().0 || sell_price.unwrap().1 < price {
                        positions_to_cancel.push(position.clone());
                        continue;
                    }
                }
            }

            if let Some(ref open_position) = open_position {
                let open_price = open_position.average_open_price();
                let (high_price, low_price) = if open_price > current_price {
                    (open_price, current_price)
                } else {
                    (current_price, open_price)
                };

                if position.position_type() == PositionType::Short {
                    if position.ordered_price() <= high_price {
                        positions_to_cancel.push(position.clone());
                        continue;
                    }
                } else {
                    if position.ordered_price() >= low_price {
                        positions_to_cancel.push(position.clone());
                        continue;
                    }
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

    fn find_min_max_trade_prices(
        actions: &[TradeAction],
    ) -> (Option<(f64, f64)>, Option<(f64, f64)>) {
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

        let buy_price = match (min_buy_price, max_buy_price) {
            (Some(min), Some(max)) => Some((min, max)),
            _ => None,
        };

        let sell_price = match (min_sell_price, max_sell_price) {
            (Some(min), Some(max)) => Some((min, max)),
            _ => None,
        };

        (buy_price, sell_price)
    }

    pub async fn liquidate(&mut self, reason: Option<String>) {
        self.statistics.liquidate_count += 1;

        let res = self
            .state
            .dex_connector
            .cancel_all_orders(Some(self.config.token_name.clone()))
            .await;

        if let Err(e) = res {
            log::error!("liquidate failed (cancel): {:?}", e);
        }

        let res = self.state.dex_connector.close_all_positions(None).await;

        for (_, position) in self.state.trade_positions.iter_mut() {
            let _ = position.on_closed(None, 0.0, true, reason.clone());
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
        }
    }

    pub fn reset_dex_client(&mut self, dex_connector: Arc<DexConnectorBox>) {
        self.state.dex_connector = dex_connector;
    }
}
