// fund_manager.rs

use super::dex_connector_box::DexConnectorBox;
use super::DBHandler;
use debot_market_analyzer::{MarketData, TradeAction, TradingStrategy};
use debot_position_manager::{PositionType, ReasonForClose, State, TradePosition};
use dex_connector::{CreateOrderResponse, DexConnector, DexError, OrderSide};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct TradeChance {
    pub action: TradeAction,
    pub token_name: String,
    pub predicted_price: Option<Decimal>,
    pub token_amount: Decimal,
    pub atr: Option<Decimal>,
    pub position_id: Option<u32>,
}

struct FundManagerState {
    amount: Decimal,
    trade_positions: HashMap<u32, TradePosition>,
    latest_open_position_id: Option<u32>,
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    market_data: MarketData,
    last_position_close_time: Option<i64>,
    last_price: Decimal,
    min_tick: Option<Decimal>,
}

struct FundManagerConfig {
    fund_name: String,
    index: usize,
    token_name: String,
    strategy: TradingStrategy,
    risk_reward: Decimal,
    trading_amount: Decimal,
    initial_amount: Decimal,
    save_prices: bool,
    order_effective_duration_secs: i64,
    execution_delay_secs: i64,
    use_market_order: bool,
    loss_cut_ratio: Decimal,
}

#[derive(Default)]
struct FundManagerStatics {
    order_count: u32,
    fill_count: u32,
    take_profit_count: u32,
    cut_loss_count: u32,
    market_make_count: u32,
    market_make_fail_count: u32,
    pnl: Decimal,
    min_amount: Decimal,
}

pub struct FundManager {
    config: FundManagerConfig,
    state: FundManagerState,
    statistics: FundManagerStatics,
}

impl FundManager {
    pub fn new(
        fund_name: &str,
        index: usize,
        token_name: &str,
        open_positions: Option<HashMap<u32, TradePosition>>,
        market_data: MarketData,
        strategy: TradingStrategy,
        trading_amount: Decimal,
        initial_amount: Decimal,
        risk_reward: Decimal,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_connector: Arc<DexConnectorBox>,
        save_prices: bool,
        order_effective_duration_secs: i64,
        use_market_order: bool,
        loss_cut_ratio: Decimal,
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
            order_effective_duration_secs,
            use_market_order,
            loss_cut_ratio,
            execution_delay_secs: order_effective_duration_secs,
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
            last_position_close_time: None,
            latest_open_position_id: None,
            last_price: Decimal::new(0, 0),
            min_tick: None,
        };

        let mut statistics = FundManagerStatics::default();
        statistics.min_amount = initial_amount;

        Self {
            config,
            state,
            statistics,
        }
    }

    pub async fn initialize(&self, leverage: u32) {
        if self
            .state
            .dex_connector
            .set_leverage(self.token_name(), leverage)
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

    pub async fn get_token_price(
        &mut self,
    ) -> Result<(Decimal, Decimal), Box<dyn Error + Send + Sync>> {
        let token_name = &self.config.token_name;

        // Get the token price
        let dex_connector = self.state.dex_connector.clone();

        // Get the token price
        let res = dex_connector
            .get_ticker(token_name)
            .await
            .map_err(|e| format!("Failed to get price of {}: {:?}", token_name, e).to_owned())?;

        if res.min_tick.is_none() {
            return Err(format!("min_tick is not available").into());
        }

        Ok((res.price, res.min_tick.unwrap()))
    }

    pub fn set_min_tick(&mut self, min_tick: Decimal) {
        if self.state.min_tick.is_none() {
            self.state.min_tick = Some(min_tick);
        }
    }

    pub async fn find_chances(
        &mut self,
        price: Decimal,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Add price
        let rounded_price = Self::round_price(price, self.state.min_tick);
        let data = &mut self.state.market_data;
        let price_point = data.add_price(Some(rounded_price), None);

        // Save the price in the DB
        if self.config.index == 0 && self.config.save_prices {
            log::trace!(
                "{}: price = {:.5}, min_tick = {:.5?}, rounded_price = {:.5}",
                self.config.token_name,
                price,
                self.state.min_tick,
                price_point.price
            );

            self.state
                .db_handler
                .lock()
                .await
                .log_price(data.name(), &self.config.token_name, price_point)
                .await;
        }

        // Add last trade price
        if self.config.strategy == TradingStrategy::MarketMake {
            let last_trade_prices = self
                .state
                .dex_connector
                .get_last_trades(&self.config.token_name)
                .await?;

            for last_trade in last_trade_prices.last_trades {
                let rounded_last_trade_price =
                    Self::round_price(last_trade.price, self.state.min_tick);
                data.add_transaction_price(rounded_last_trade_price);
            }

            self.state
                .dex_connector
                .clear_last_trades(&self.config.token_name)
                .await?;
        }

        self.find_expired_orders().await;

        self.find_close_chances(price)
            .await
            .map_err(|_| "Failed to find close chances".to_owned())?;

        self.find_open_chances(price, rounded_price)
            .await
            .map_err(|_| "Failed to find open chances".to_owned())?;
        self.state.last_price = price;

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

        // Cancel the exipired orders
        for position in &positions_to_cancel {
            log::debug!("Canceling expired order: order_id:{}", position.order_id());
            self.cancel_order(position.order_id(), false).await;
        }
    }

    async fn find_open_chances(
        &mut self,
        current_price: Decimal,
        rounded_price: Decimal,
    ) -> Result<(), ()> {
        if self.config.trading_amount == Decimal::new(0, 0) {
            return Ok(());
        }

        match self.config.strategy {
            TradingStrategy::TrendFollow(_) => {
                if let Some(last_close_time) = self.state.last_position_close_time {
                    let current_time = chrono::Utc::now().timestamp();
                    let delay_secs = self.config.execution_delay_secs;
                    if current_time - last_close_time < delay_secs {
                        log::info!(
                            "Waiting for delay period to pass before executing new chances."
                        );
                        return Ok(());
                    }
                }
            }
            TradingStrategy::MarketMake => match self.state.trade_positions.len() {
                0 => {}
                1 => {
                    self.close_open_position().await;
                    self.statistics.market_make_fail_count += 1;
                }
                _ => {
                    let actions: Vec<TradeAction> = vec![];
                    return self.handle_open_chances(current_price, &actions).await;
                }
            },
        }

        let actions: Vec<TradeAction> = self.state.market_data.is_open_signaled(
            self.config.strategy.clone(),
            rounded_price,
            self.config.trading_amount,
        );

        self.handle_open_chances(current_price, &actions).await
    }

    async fn handle_open_chances(
        &mut self,
        current_price: Decimal,
        actions: &Vec<TradeAction>,
    ) -> Result<(), ()> {
        const _GREEN: &str = "\x1b[0;32m";
        const RED: &str = "\x1b[0;31m";
        const GREY: &str = "\x1b[0;90m";
        const RESET: &str = "\x1b[0m";
        const BLUE: &str = "\x1b[0;34m";
        const LIGHT_RED: &str = "\x1b[1;31m";
        const LIGHT_BLUE: &str = "\x1b[1;34m";

        let data = self.state.market_data.to_owned();

        let target_price_factor = self.config.loss_cut_ratio * self.config.risk_reward;
        let mut order_price = current_price;
        let use_market_order = self.config.use_market_order;

        for action in actions.clone() {
            let is_buy;
            let (target_price, token_amount, confidence) = match action.clone() {
                TradeAction::BuyOpen(detail) => {
                    is_buy = true;
                    (
                        detail.target_price(),
                        detail.token_amount(),
                        detail.confidence(),
                    )
                }
                TradeAction::SellOpen(detail) => {
                    is_buy = false;
                    (
                        detail.target_price(),
                        detail.token_amount(),
                        detail.confidence(),
                    )
                }
                _ => continue,
            };
            let mut token_amount = match token_amount {
                Some(token_amount) => token_amount * confidence,
                None => self.config.trading_amount * confidence,
            };

            let decimal_1 = Decimal::new(1, 0);
            let target_price = match self.config.strategy {
                TradingStrategy::MarketMake => {
                    order_price = target_price.unwrap();
                    if is_buy {
                        order_price * (decimal_1 + target_price_factor)
                    } else {
                        order_price * (decimal_1 - target_price_factor)
                    }
                }
                _ => {
                    if is_buy {
                        current_price * (decimal_1 + target_price_factor)
                    } else {
                        current_price * (decimal_1 - target_price_factor)
                    }
                }
            };

            if self.state.amount <= token_amount * order_price {
                log::warn!("No enough fund left: {:.6}", self.state.amount);
                if self.state.amount > Decimal::new(0, 0) {
                    token_amount = self.state.amount / order_price;
                } else {
                    break;
                }
            }

            self.execute_chances(
                order_price,
                TradeChance {
                    token_name: self.config.token_name.clone(),
                    predicted_price: Some(target_price),
                    token_amount,
                    atr: Some(data.atr()),
                    action,
                    position_id: None,
                },
                None,
                use_market_order,
            )
            .await?;
        }

        let mut positions_vec: Vec<TradePosition> = self
            .state
            .trade_positions
            .iter()
            .map(|(_, v)| v.clone())
            .collect();

        let decimal_0 = Decimal::new(0, 0);
        let dummy_position = TradePosition::new(
            0,
            "",
            current_price,
            decimal_0,
            0,
            "",
            "",
            PositionType::Long,
            decimal_0,
            None,
        );
        positions_vec.push(dummy_position);

        positions_vec.sort_by_key(|v| v.ordered_price());

        if positions_vec.len() > 1 {
            for position in positions_vec.iter().rev() {
                if matches!(
                    position.state(),
                    State::Opening | State::Open | State::Closing(_)
                ) {
                    let (mut side, mut color) = match position.position_type() {
                        PositionType::Long => (format!("{:4}", "Buy"), BLUE),
                        PositionType::Short => (format!("{:4}", "Sell"), RED),
                    };
                    if position.state() == State::Open {
                        if position.position_type() == PositionType::Long {
                            color = LIGHT_BLUE;
                        } else {
                            color = LIGHT_RED;
                        }
                    }
                    if position.id().unwrap() == 0 {
                        side = format!("{:4}", "");
                        color = GREY;
                    }

                    let is_updated = actions
                        .iter()
                        .any(|a| a.target_price().unwrap_or_default() == position.ordered_price());

                    let amount = format!(
                        "{:6.6}",
                        if position.state() == State::Open {
                            position.amount()
                        } else {
                            position.unfilled_amount()
                        }
                    );

                    log::debug!(
                        "{:<5}: {}{:<4.4}{}({}){:1} {}",
                        side,
                        color,
                        position.ordered_price(),
                        RESET,
                        amount,
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
                        },
                        if position.order_id() == "" {
                            String::new()
                        } else {
                            format!(
                                "[{},{}]",
                                position.order_id(),
                                position.id().unwrap_or_default()
                            )
                        }
                    );
                }
            }
        }

        let (pnl, ratio) = self.unrealized_pnl_of_open_position(current_price);

        match self.config.strategy {
            TradingStrategy::TrendFollow(_) => {
                log::info!(
                    "{} pnl: {:.3}/{:.3}({:.3}%) order/fill/profit = {}/{}/{}, min position = {:.1}, trend = {:?}",
                    format!("{}-{}", self.config.token_name, self.config.index),
                    self.statistics.pnl,
                    pnl,
                    ratio * Decimal::new(100, 0),
                    self.statistics.order_count,
                    self.statistics.fill_count,
                    self.statistics.take_profit_count,
                    self.statistics.min_amount,
                    self.state.market_data.trend()
                );
            }
            TradingStrategy::MarketMake => {
                log::info!(
                    "{} pnl: {:.3}/{:.3}({:.3}%) make/fail = {}/{}",
                    format!("{}-{}", self.config.token_name, self.config.index),
                    self.statistics.pnl,
                    pnl,
                    ratio * Decimal::new(100, 0),
                    self.statistics.market_make_count / 2,
                    self.statistics.market_make_fail_count,
                );
            }
        }

        Ok(())
    }

    async fn find_close_chances(&mut self, current_price: Decimal) -> Result<(), ()> {
        let cloned_open_positions = self.state.trade_positions.clone();

        for (position_id, position) in cloned_open_positions.iter() {
            match position.state() {
                State::Opening => {
                    if position.amount() == Decimal::new(0, 0) {
                        continue;
                    }
                }
                State::Open => {}
                _ => continue,
            }
            let action = self
                .state
                .market_data
                .is_close_signaled(self.config.strategy.clone());

            self.handle_close_chances(current_price, *position_id, position, &action)
                .await?;
        }

        Ok(())
    }

    async fn handle_close_chances(
        &mut self,
        current_price: Decimal,
        position_id: u32,
        position: &TradePosition,
        action: &TradeAction,
    ) -> Result<(), ()> {
        let mut reason_for_close = match action {
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
            _ => None,
        };

        if reason_for_close.is_none() {
            reason_for_close = position.should_close(current_price);
        }

        let mut chance: Option<TradeChance> = None;

        if reason_for_close.is_some() {
            chance = Some(TradeChance {
                token_name: self.config.token_name.clone(),
                predicted_price: None,
                token_amount: position.amount().abs(),
                atr: None,
                action: if position.position_type() == PositionType::Long {
                    TradeAction::SellClose
                } else {
                    TradeAction::BuyClose
                },
                position_id: Some(position_id),
            });
        }

        if let Some(chance) = chance {
            self.execute_chances(
                current_price,
                chance,
                reason_for_close.clone(),
                self.config.use_market_order,
            )
            .await?;

            self.state.last_position_close_time = Some(chrono::Utc::now().timestamp());
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn is_single_order_type(&self) -> bool {
        let filtered_positions: Vec<_> = self
            .state
            .trade_positions
            .values()
            .filter(|p| !matches!(p.state(), State::Closing(_)))
            .filter(|p| {
                p.asset_in_usd() == Decimal::new(0, 0)
                    || p.asset_in_usd().abs() >= self.config.trading_amount / Decimal::new(2, 0)
            })
            .collect();

        if filtered_positions.is_empty() {
            return false;
        }

        let first_position_type = filtered_positions[0].position_type();

        filtered_positions
            .iter()
            .all(|p| p.position_type() == first_position_type)
    }

    async fn execute_chances(
        &mut self,
        order_price: Decimal,
        chance: TradeChance,
        reason_for_close: Option<ReasonForClose>,
        use_market_order: bool,
    ) -> Result<(), ()> {
        if chance.token_amount <= Decimal::new(0, 0) {
            log::error!(
                "execute_chance: wrong token amount: {}",
                chance.token_amount
            );
            return Err(());
        }

        let symbol = &self.config.token_name;
        let size = chance.token_amount;
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
            "Execute: {} {} [{}, {}] order_price = {:<6.4?}, size = {:.10}",
            format!("{}-{}", self.config.token_name, self.config.index),
            if chance.action.is_open() {
                "Open"
            } else {
                "Close"
            },
            side,
            reason,
            order_price,
            size,
        );

        // Execute the transaction
        let order_price = match reason_for_close {
            Some(ReasonForClose::Liquidated) | None if use_market_order => None,
            _ => Some(order_price),
        };

        let res: Result<CreateOrderResponse, DexError> = self
            .state
            .dex_connector
            .create_order(symbol, size, side.clone(), order_price, Some(1))
            .await;
        match res {
            Ok(res) => {
                if res.ordered_size > Decimal::new(0, 0) {
                    let order_id = res.order_id;
                    self.prepare_position(
                        &order_id,
                        if res.ordered_price == Decimal::new(0, 0) {
                            None
                        } else {
                            Some(res.ordered_price)
                        },
                        res.ordered_size,
                        chance.action,
                        chance.predicted_price,
                        reason_for_close,
                        &chance.token_name,
                        chance.atr,
                        chance.position_id,
                    )
                    .await?;
                }
            }
            Err(e) => {
                log::info!(
                    "create_order failed({}, {}, {:?}): {:?}",
                    symbol,
                    size,
                    side,
                    e
                );
            }
        }

        Ok(())
    }

    async fn prepare_position(
        &mut self,
        order_id: &str,
        ordered_price: Option<Decimal>,
        ordered_amount: Decimal,
        trade_action: TradeAction,
        predicted_price: Option<Decimal>,
        reason_for_close: Option<ReasonForClose>,
        token_name: &str,
        atr: Option<Decimal>,
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
                log::warn!(
                    "prepare_positino: position not found: position_id = {}",
                    position_id
                );
                return Err(());
            }
            let position = position.unwrap();
            position.request_close(order_id, &reason_for_close.clone().unwrap().to_string())?;
            position_cloned = position.clone();

            if let Some(reason) = reason_for_close {
                match reason {
                    ReasonForClose::TakeProfit => self.statistics.take_profit_count += 1,
                    ReasonForClose::CutLoss => self.statistics.cut_loss_count += 1,
                    _ => {}
                }
            }
        }

        self.statistics.order_count += 1;

        if self.config.strategy == TradingStrategy::MarketMake {
            self.statistics.market_make_count += 1;
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

    fn get_open_position(&self) -> Option<TradePosition> {
        match self.state.latest_open_position_id {
            Some(id) => self.state.trade_positions.get(&id).cloned(),
            None => None,
        }
    }

    #[allow(dead_code)]
    fn pnl_of_open_position(&self) -> Decimal {
        match self.get_open_position() {
            Some(position) => position.pnl(),
            None => Decimal::new(0, 0),
        }
    }

    fn unrealized_pnl_of_open_position(&self, price: Decimal) -> (Decimal, Decimal) {
        match self.get_open_position() {
            Some(position) => {
                let pnl = position.amount() * price + position.asset_in_usd();
                let ratio = if position.asset_in_usd().abs().is_sign_positive() {
                    pnl / position.asset_in_usd().abs()
                } else {
                    Decimal::new(0, 0)
                };
                (pnl, ratio)
            }
            None => (Decimal::new(0, 0), Decimal::new(0, 0)),
        }
    }

    fn process_trade_position(
        &mut self,
        order_position_id: &u32,
        open_position_id: Option<u32>,
        position_type: PositionType,
        filled_price: Decimal,
        filled_size: Decimal,
        filled_value: Decimal,
        fee: Decimal,
        take_profit_price: Option<Decimal>,
        cut_loss_price: Option<Decimal>,
    ) -> Result<(), ()> {
        let position_cloned;

        // step 1: fill the order position
        let position_id = match open_position_id {
            Some(open_position_id) => {
                if open_position_id == *order_position_id {
                    None
                } else {
                    Some(order_position_id)
                }
            }
            None => Some(order_position_id),
        };

        if let Some(position_id) = position_id {
            let position = self
                .state
                .trade_positions
                .get_mut(order_position_id)
                .ok_or_else(|| {
                    log::error!(
                        "process_trade_position: position not found: order_position_id = {}",
                        position_id,
                    );
                    ()
                })?;

            log::debug!(
                "step 1: process_trade_position: on_filled for order_position: {}",
                position_id
            );

            position.on_filled(
                position_type.clone(),
                filled_price,
                filled_size,
                filled_value,
                fee,
                take_profit_price,
                cut_loss_price,
                self.state.market_data.last_price(),
            )?;
            position_cloned = Some(position.clone());
            if position.state() == State::Open {
                self.state.trade_positions.remove(position_id);
            }
        } else {
            position_cloned = None;
        }

        // step 2: handle a new or existing open position
        match open_position_id {
            Some(open_position_id) => match self.state.trade_positions.get_mut(&open_position_id) {
                Some(open_position) => {
                    log::debug!(
                        "step 2: process_trade_position: on_filled for open_position: {}",
                        open_position_id
                    );

                    open_position.on_filled(
                        position_type,
                        filled_price,
                        filled_size,
                        filled_value,
                        fee,
                        take_profit_price,
                        cut_loss_price,
                        self.state.market_data.last_price(),
                    )?;
                }
                None => {
                    log::error!(
                        "process_trade_position: open position not found: id = {}",
                        open_position_id
                    );
                    return Err(());
                }
            },
            None => {
                let position_cloned = position_cloned.unwrap();
                if position_cloned.state() == State::Open {
                    self.state
                        .trade_positions
                        .insert(*order_position_id, position_cloned);
                    self.state.latest_open_position_id = Some(*order_position_id);
                    log::debug!(
                        "process_trade_position: new open_position_id = {:?}",
                        self.state.latest_open_position_id
                    );
                }
            }
        }

        Ok(())
    }

    fn update_state_after_trade(&mut self, filled_value: Decimal) {
        match self.state.latest_open_position_id {
            Some(position_id) => {
                let position = self.state.trade_positions.get(&position_id).unwrap();
                let position_asset = position.asset_in_usd();
                self.state.amount = if position_asset.is_sign_positive() {
                    self.config.initial_amount - position_asset
                } else {
                    self.config.initial_amount + position_asset
                }
            }
            None => self.state.amount -= filled_value,
        }
    }

    pub async fn clear_filled_order(&self, trade_id: &str) {
        let _ = self
            .state
            .dex_connector
            .clear_filled_order(&self.config.token_name, &trade_id)
            .await
            .map_err(|e| {
                log::error!("{:?}", e);
                ()
            });
    }

    pub async fn position_filled(
        &mut self,
        order_id: &str,
        filled_side: OrderSide,
        filled_value: Decimal,
        filled_size: Decimal,
        fee: Decimal,
    ) -> Result<bool, ()> {
        let position = match self.find_position_from_order_id(order_id) {
            Some(p) => {
                if matches!(p.state(), State::Open) {
                    log::info!("Ignore already filled order: {:?}", p);
                    return Ok(false);
                }
                p
            }
            None => {
                log::warn!("Filled position not found: order_id = {}", order_id,);
                return Ok(false);
            }
        };

        let predicted_price = position.predicted_price();
        let position_type = match filled_side {
            OrderSide::Long => PositionType::Long,
            OrderSide::Short => PositionType::Short,
        };

        let position_id = match position.id() {
            Some(p) => p,
            None => {
                log::error!("position id is None: order_id = {}", order_id);
                return Ok(false);
            }
        };

        let filled_price = filled_value / filled_size;

        log::info!(
            "fill_position:{}, [{}] order_id = {:?}, value = {:.4?}, size = {:.10?}, fee = {:.4?}, price = {:<6.6}",
            self.config.token_name,
            filled_side,
            order_id,
            filled_value,
            filled_size,
            fee,
            filled_price,
        );

        let prev_amount = self.state.amount;
        let distance = (predicted_price - filled_price).abs() / self.config.risk_reward;
        let cut_loss_price = match self.config.strategy {
            TradingStrategy::TrendFollow(_) | TradingStrategy::MarketMake => None,
            #[allow(unreachable_patterns)]
            _ => match filled_side {
                OrderSide::Long => Some(filled_price - distance),
                _ => Some(filled_price + distance),
            },
        };
        let take_profit_price = match self.config.strategy {
            TradingStrategy::MarketMake => None,
            _ => Some(predicted_price),
        };

        let open_position_id = self.state.latest_open_position_id;

        self.process_trade_position(
            &position_id,
            open_position_id,
            position_type,
            filled_price,
            filled_size,
            filled_value,
            fee,
            take_profit_price,
            cut_loss_price,
        )?;

        self.update_state_after_trade(filled_value);

        if let Some(position) = self.get_open_position() {
            if matches!(position.state(), State::Closed(_)) {
                self.state.amount += position.close_asset_in_usd() + position.pnl();
                self.state.latest_open_position_id = None;
                self.state
                    .trade_positions
                    .remove(&position.id().unwrap_or_default());
                self.statistics.pnl += position.pnl();
            }

            // Save the position in the DB
            self.state
                .db_handler
                .lock()
                .await
                .log_position(&position)
                .await;
        }

        self.statistics.fill_count += 1;

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

    pub async fn cancel_order(&mut self, order_id: &str, is_already_rejected: bool) {
        if !is_already_rejected {
            if let Err(e) = self
                .state
                .dex_connector
                .cancel_order(&self.config.token_name, order_id)
                .await
            {
                log::error!("{:?}", e);
                return;
            }
        }

        let position = match self.find_position_from_order_id(&order_id) {
            Some(v) => v,
            None => {
                log::error!("cancel_order: position not found: order_id = {}", order_id);
                return;
            }
        };

        let position = self
            .state
            .trade_positions
            .get_mut(&position.id().unwrap())
            .unwrap();

        let cancel_result = match position.cancel() {
            Ok(cancel_result) => cancel_result,
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

        let position_id = match position.id() {
            Some(id) => id,
            None => {
                log::error!("Position id is None: {:?}", position);
                return;
            }
        };

        match cancel_result {
            debot_position_manager::CancelResult::OpeningCanceled => {
                // Opening --> Canceled
                self.state.trade_positions.remove(&position_id);
            }
            debot_position_manager::CancelResult::ClosingCanceled => {
                // Closing --> Open
            }
            debot_position_manager::CancelResult::PartiallyFilled => {
                // Opening --> Open
                if self.state.latest_open_position_id.is_none() {
                    self.state.latest_open_position_id = Some(position_id);
                } else {
                    // Ignore the paritally filled position
                    self.state.trade_positions.remove(&position_id);
                }
            }
        }
    }

    #[allow(dead_code)]
    async fn cancel_all_orders(&mut self) {
        let _ = self
            .state
            .dex_connector
            .cancel_all_orders(Some(self.config.token_name.clone()))
            .await
            .map_err(|e| {
                log::error!("cancel_all_orders: {:?}", e);
            });

        match self.state.latest_open_position_id {
            Some(open_position_id) => {
                self.state
                    .trade_positions
                    .retain(|position_id, _| *position_id == open_position_id);
            }
            None => {
                self.state.trade_positions.clear();
            }
        }
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

        let res = self.state.dex_connector.close_all_positions(None).await;

        for (_, position) in self.state.trade_positions.iter_mut() {
            let _ = position.on_liquidated(
                self.state.market_data.last_price(),
                Decimal::new(0, 0),
                true,
                reason.clone(),
            );
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

    fn round_price(price: Decimal, min_tick: Option<Decimal>) -> Decimal {
        let min_tick = min_tick.unwrap_or(Decimal::ONE);
        (price / min_tick).round() * min_tick
    }

    async fn close_open_position(&mut self) {
        if let Some(open_position) = self.get_open_position() {
            if matches!(open_position.state(), State::Open) {
                let _ = self
                    .execute_chances(
                        self.state.market_data.last_price(),
                        TradeChance {
                            token_name: self.config.token_name.clone(),
                            predicted_price: None,
                            token_amount: open_position.amount().abs(),
                            atr: None,
                            action: if open_position.position_type() == PositionType::Long {
                                TradeAction::SellClose
                            } else {
                                TradeAction::BuyClose
                            },
                            position_id: open_position.id(),
                        },
                        Some(ReasonForClose::Other(String::from(
                            "OnlyOneSideOrderFilled",
                        ))),
                        self.config.use_market_order,
                    )
                    .await;
            }
        }
    }

    pub fn check_positions(&self, price: Decimal) {
        for (_, position) in &self.state.trade_positions {
            position.print_info(price);
        }
    }

    pub fn reset_dex_client(&mut self, dex_connector: Arc<DexConnectorBox>) {
        self.state.dex_connector = dex_connector;
    }
}
