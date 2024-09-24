// fund_manager.rs

use super::dex_connector_box::DexConnectorBox;
use super::DBHandler;
use debot_db::{CandlePattern, PricePoint};
use debot_market_analyzer::{MarketData, SampleTerm, TradeAction, TradeDetail, TradingStrategy};
use debot_position_manager::{PositionType, ReasonForClose, State, TradePosition};
use debot_utils::is_sunday;
use dex_connector::{CreateOrderResponse, DexConnector, DexError, OrderSide};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone)]
struct TradeChance {
    pub action: TradeAction,
    pub token_name: String,
    pub target_price: Option<Decimal>,
    pub token_amount: Decimal,
    pub position_id: Option<u32>,
}

struct FundManagerState {
    amount: Decimal,
    trade_positions: HashMap<u32, TradePosition>,
    latest_open_position_id: Option<u32>,
    db_handler: Arc<Mutex<DBHandler>>,
    dex_connector: Arc<DexConnectorBox>,
    market_data: Arc<RwLock<MarketData>>,
    last_trade_time: Option<i64>,
    last_price: Decimal,
}

struct FundManagerConfig {
    fund_name: String,
    index: usize,
    token_name: String,
    strategy: TradingStrategy,
    trading_amount: Decimal,
    initial_amount: Decimal,
    open_order_tick_count_max: u32,
    close_order_tick_count_max: u32,
    open_tick_count_max: u32,
    execution_delay_secs: i64,
    use_market_order: bool,
    take_profit_ratio: Option<Decimal>,
    risk_reward: Decimal,
    atr_spread: Option<Decimal>,
    atr_term: SampleTerm,
}

#[derive(Default)]
struct FundManagerStatics {
    order_count: i32,
    fill_count: i32,
    take_profit_count: i32,
    cut_loss_count: i32,
    trim_count: i32,
    trend_changed_count: i32,
    expired_count: i32,
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
        market_data: Arc<RwLock<MarketData>>,
        strategy: TradingStrategy,
        trading_amount: Decimal,
        initial_amount: Decimal,
        db_handler: Arc<Mutex<DBHandler>>,
        dex_connector: Arc<DexConnectorBox>,
        open_order_tick_count_max: u32,
        close_order_tick_count_max: u32,
        open_tick_count_max: u32,
        execution_delay_secs: i64,
        use_market_order: bool,
        take_profit_ratio: Option<Decimal>,
        risk_reward: Decimal,
        atr_spread: Option<Decimal>,
        atr_term: SampleTerm,
    ) -> Self {
        let config = FundManagerConfig {
            fund_name: fund_name.to_owned(),
            index,
            token_name: token_name.to_owned(),
            strategy,
            trading_amount,
            initial_amount,
            open_order_tick_count_max,
            close_order_tick_count_max,
            open_tick_count_max,
            execution_delay_secs,
            use_market_order,
            take_profit_ratio,
            risk_reward,
            atr_spread,
            atr_term,
        };

        log::info!("initial amount = {}", initial_amount);

        let state = FundManagerState {
            amount: initial_amount,
            trade_positions: HashMap::new(),
            db_handler,
            dex_connector,
            market_data,
            last_trade_time: None,
            latest_open_position_id: None,
            last_price: Decimal::new(0, 0),
        };

        let mut statistics = FundManagerStatics::default();
        statistics.min_amount = initial_amount;

        Self {
            config,
            state,
            statistics,
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
        back_test_price: Option<&PricePoint>,
    ) -> Result<
        (
            Decimal,
            Decimal,
            Option<i64>,
            Option<Decimal>,
            Option<u64>,
            Option<Decimal>,
            Option<Decimal>,
            Option<Decimal>,
        ),
        Box<dyn Error + Send + Sync>,
    > {
        let token_name = &self.config.token_name;
        let dex_connector = self.state.dex_connector.clone();

        // Get the token price
        let test_price = back_test_price.and_then(|test_price| Some(test_price.price));
        let timestamp = back_test_price.and_then(|test_price| Some(test_price.timestamp));
        let res = dex_connector
            .get_ticker(token_name, test_price)
            .await
            .map_err(|e| format!("Failed to get price of {}: {:?}", token_name, e).to_owned())?;

        if res.min_tick.is_none() {
            return Err(format!("min_tick is not available").into());
        }

        Ok((
            res.price,
            res.min_tick.unwrap(),
            timestamp,
            res.volume,
            res.num_trades,
            res.funding_rate,
            res.open_interest,
            res.oracle_price,
        ))
    }

    pub async fn find_chances(
        &mut self,
        price: Decimal,
        dry_run: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.check_positions(price);

        self.find_expired_orders().await;

        self.find_close_chances(price)
            .await
            .map_err(|_| "Failed to find close chances".to_owned())?;

        self.find_open_chances(price, dry_run)
            .await
            .map_err(|_| "Failed to find open chances".to_owned())?;
        self.state.last_price = price;

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

    async fn find_open_chances(&mut self, current_price: Decimal, dry_run: bool) -> Result<(), ()> {
        if self.config.trading_amount == Decimal::new(0, 0) {
            return Ok(());
        }

        let mut actions: Vec<TradeAction> = vec![];

        match self.config.strategy {
            TradingStrategy::RandomWalk(_) | TradingStrategy::MeanReversion(_) => {
                if !self.can_execute_new_trade() {
                    return self.handle_open_chances(current_price, &actions).await;
                }
            }
        }

        if dry_run || !is_sunday() {
            actions = self.state.market_data.read().await.is_open_signaled(
                self.config.strategy.clone(),
                self.config.take_profit_ratio.unwrap_or_default(),
                self.config.atr_spread,
                self.config.open_order_tick_count_max,
                &self.config.atr_term,
            );
        }

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

        for action in actions.clone() {
            let is_buy;
            let (order_price, token_amount, confidence) = match action.clone() {
                TradeAction::BuyOpen(detail) => {
                    is_buy = true;
                    (
                        detail.order_price(),
                        detail.amount_in_usd(),
                        detail.confidence(),
                    )
                }
                TradeAction::SellOpen(detail) => {
                    is_buy = false;
                    (
                        detail.order_price(),
                        detail.amount_in_usd(),
                        detail.confidence(),
                    )
                }
                _ => continue,
            };

            let side = if is_buy {
                OrderSide::Long
            } else {
                OrderSide::Short
            };
            let order_price = match self.order_price(current_price, order_price, is_buy).await {
                Ok(order_price) => order_price,
                Err(_) => continue,
            };
            let token_amount = match token_amount {
                Some(token_amount) => token_amount * confidence,
                None => self.config.trading_amount / order_price * confidence,
            };
            let target_price = self.target_price(current_price, side, false).await;
            if target_price.is_none() {
                continue;
            }

            if self.state.amount <= token_amount * order_price {
                log::warn!(
                    "{} does not have enough fund: {:.6}",
                    self.config.fund_name,
                    self.state.amount
                );
                continue;
            }

            self.execute_chances(
                order_price,
                TradeChance {
                    token_name: self.config.token_name.clone(),
                    target_price,
                    token_amount,
                    action,
                    position_id: None,
                },
                None,
            )
            .await?;
        }

        if self.state.trade_positions.is_empty() {
            return Ok(());
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
            "",
            current_price,
            decimal_0,
            0,
            0,
            0,
            "",
            PositionType::Long,
            Decimal::ZERO,
            (
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            (
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            (
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            (
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            (
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ),
            (
                CandlePattern::None,
                CandlePattern::None,
                CandlePattern::None,
                CandlePattern::None,
            ),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            None,
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
                    if position.id() == 0 {
                        side = format!("{:4}", "");
                        color = GREY;
                    }

                    let is_updated = actions
                        .iter()
                        .any(|a| a.order_price().unwrap_or_default() == position.ordered_price());

                    let amount_value = if position.state() == State::Opening {
                        position.unfilled_amount()
                    } else {
                        position.amount()
                    };

                    let amount = format!("{:6.6}", amount_value);

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
                            format!("[{},{}]", position.order_id(), position.id(),)
                        }
                    );
                }
            }
        }

        let (pnl, ratio) = self.unrealized_pnl_of_open_position(current_price);

        match self.config.strategy {
            TradingStrategy::RandomWalk(_) | TradingStrategy::MeanReversion(_) => {
                log::info!(
                    "{} pnl: {:.3}/{:.3}({:.3}%) {}/{}/{}",
                    format!(
                        "{}-{}-{}",
                        self.config.token_name,
                        self.config.take_profit_ratio.unwrap_or_default(),
                        self.config.atr_spread.unwrap_or_default()
                    ),
                    self.statistics.pnl,
                    pnl,
                    ratio * Decimal::new(100, 0),
                    self.statistics.take_profit_count,
                    self.statistics.cut_loss_count,
                    self.statistics.expired_count,
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
            let action = self.state.market_data.read().await.is_close_signaled(
                self.config.strategy.clone(),
                position.asset_in_usd().abs(),
                self.is_profitable_position(*position_id).await,
            );

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
        let mut confidence = Decimal::ONE;
        let mut reason_for_close = match action {
            TradeAction::BuyClose(_) => {
                if position.position_type() == PositionType::Short {
                    self.statistics.trend_changed_count += 1;
                    confidence = action.confidence().unwrap_or_default();
                    self.cancel_all_orders().await;
                    Some(ReasonForClose::Other("TredeChanged".to_owned()))
                } else {
                    None
                }
            }
            TradeAction::SellClose(_) => {
                if position.position_type() == PositionType::Long {
                    self.statistics.trend_changed_count += 1;
                    confidence = action.confidence().unwrap_or_default();
                    self.cancel_all_orders().await;
                    Some(ReasonForClose::Other("TrendChanged".to_owned()))
                } else {
                    None
                }
            }
            TradeAction::BuyTrim(_) => {
                if position.position_type() == PositionType::Short {
                    self.statistics.trim_count += 1;
                    confidence = action.confidence().unwrap_or_default();
                    self.cancel_all_orders().await;
                    Some(ReasonForClose::Other("TrimPosition".to_owned()))
                } else {
                    None
                }
            }
            TradeAction::SellTrim(_) => {
                if position.position_type() == PositionType::Long {
                    self.statistics.trim_count += 1;
                    confidence = action.confidence().unwrap_or_default();
                    self.cancel_all_orders().await;
                    Some(ReasonForClose::Other("TrimPosition".to_owned()))
                } else {
                    None
                }
            }
            _ => None,
        };

        if reason_for_close.is_none() {
            reason_for_close = position.should_close(current_price);
            if let Some(reason) = reason_for_close.clone() {
                match reason {
                    ReasonForClose::TakeProfit => self.statistics.take_profit_count += 1,
                    ReasonForClose::CutLoss => self.statistics.cut_loss_count += 1,
                    _ => {}
                }
            } else if matches!(
                self.config.strategy,
                TradingStrategy::RandomWalk(_) | TradingStrategy::MeanReversion(_)
            ) {
                if position.should_open_expired() {
                    reason_for_close = Some(ReasonForClose::Expired);
                    self.statistics.expired_count += 1;
                }
            }
        }

        let mut chance: Option<TradeChance> = None;

        if reason_for_close.is_some() {
            chance = Some(TradeChance {
                token_name: self.config.token_name.clone(),
                target_price: None,
                token_amount: position.amount().abs() * confidence,
                action: if position.position_type() == PositionType::Long {
                    TradeAction::SellClose(TradeDetail::new(None, None, Decimal::ONE))
                } else {
                    TradeAction::BuyClose(TradeDetail::new(None, None, Decimal::ONE))
                },
                position_id: Some(position_id),
            });
        }

        if let Some(chance) = chance {
            self.execute_chances(current_price, chance, reason_for_close.clone())
                .await?;
        }

        Ok(())
    }

    fn can_execute_new_trade(&self) -> bool {
        if matches!(
            self.config.strategy,
            TradingStrategy::RandomWalk(_) | TradingStrategy::MeanReversion(_)
        ) && !self.state.trade_positions.is_empty()
        {
            return false;
        }

        match self.config.strategy {
            TradingStrategy::RandomWalk(_) | TradingStrategy::MeanReversion(_) => {
                if let Some(last_trade_time) = self.state.last_trade_time {
                    let current_time = chrono::Utc::now().timestamp();
                    let delay_secs = self.config.execution_delay_secs;
                    if current_time - last_trade_time < delay_secs {
                        log::info!(
                            "{}: Waiting for delay period to pass before executing new trades",
                            self.config.fund_name
                        );
                        return false;
                    }
                }
            }
        }

        true
    }

    async fn execute_chances(
        &mut self,
        order_price: Decimal,
        chance: TradeChance,
        reason_for_close: Option<ReasonForClose>,
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
            Some(ReasonForClose::Liquidated)
            | Some(ReasonForClose::Expired)
            | Some(ReasonForClose::CutLoss)
            | None
                if self.config.use_market_order =>
            {
                None
            }
            _ => Some(order_price),
        };

        let res: Result<CreateOrderResponse, DexError> = self
            .state
            .dex_connector
            .create_order(symbol, size, side.clone(), order_price, None)
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
                        chance.target_price,
                        reason_for_close,
                        &chance.token_name,
                        chance.position_id,
                    )
                    .await?;
                    self.state.last_trade_time = Some(chrono::Utc::now().timestamp());
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
        target_price: Option<Decimal>,
        reason_for_close: Option<ReasonForClose>,
        token_name: &str,
        position_id: Option<u32>,
    ) -> Result<(), ()> {
        let position_type = if trade_action.is_buy() {
            PositionType::Long
        } else {
            PositionType::Short
        };

        if trade_action.is_open() {
            // create a new pending position
            let id = {
                let db_handler = self.state.db_handler.lock().await;
                db_handler.increment_counter(debot_db::CounterType::Position)
            };
            if id.is_none() {
                log::error!("Failed to increment the position ID");
                return Err(());
            }

            let market_data = self.state.market_data.read().await;

            let position = TradePosition::new(
                id.unwrap(),
                &self.config.fund_name,
                order_id,
                ordered_price.unwrap(),
                ordered_amount,
                self.config.open_order_tick_count_max,
                self.config.close_order_tick_count_max,
                self.config.open_tick_count_max,
                token_name,
                position_type,
                target_price.unwrap(),
                market_data.atr(),
                market_data.adx(),
                market_data.rsi(),
                market_data.stochastic(),
                market_data.price(),
                market_data.candle_pattern(),
                self.config.take_profit_ratio.unwrap_or_default(),
                self.config.atr_spread.unwrap_or_default(),
                self.config.risk_reward,
                self.config.atr_term.to_numeric(),
                market_data.last_volume(),
                market_data.last_num_trades(),
            );

            self.state.trade_positions.insert(position.id(), position);
        } else {
            if let Some(position_id) = position_id {
                let position = self.state.trade_positions.get_mut(&position_id);
                if position.is_none() {
                    log::warn!(
                        "prepare_position: position not found: position_id = {}",
                        position_id
                    );
                    return Err(());
                }
                let position = position.unwrap();
                position.request_close(order_id, &reason_for_close.clone().unwrap().to_string())?;
            } else {
                log::warn!("prepare_position: position not found(None)");
                return Err(());
            }
        }

        self.statistics.order_count += 1;

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

    pub fn get_open_position(&self) -> Option<TradePosition> {
        match self.state.latest_open_position_id {
            Some(id) => self.state.trade_positions.get(&id).cloned(),
            None => None,
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

    async fn process_trade_position(
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
        let market_data = self.state.market_data.read().await;

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
                market_data.last_price(),
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
                        market_data.last_price(),
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

    fn update_state_after_trade(&mut self, filled_value: Decimal) -> Decimal {
        let prev_amount = self.state.amount;
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
        prev_amount
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
                    log::info!("Ignore already filled order for the position: {:?}", p);
                    return Ok(false);
                }
                p
            }
            None => {
                log::trace!(
                    "{}: Filled position not found: order_id = {}",
                    self.fund_name(),
                    order_id
                );
                return Ok(false);
            }
        };

        let target_price = position.predicted_price();
        let position_type = match filled_side {
            OrderSide::Long => PositionType::Long,
            OrderSide::Short => PositionType::Short,
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

        let cut_loss_price = self.cut_loss_price(filled_price, filled_side).await;
        let take_profit_price = self.take_profit_price(target_price);
        let open_position_id = self.state.latest_open_position_id;

        self.process_trade_position(
            &position.id(),
            open_position_id,
            position_type,
            filled_price,
            filled_size,
            filled_value,
            fee,
            take_profit_price,
            cut_loss_price,
        )
        .await?;

        let prev_amount = self.update_state_after_trade(filled_value);

        if let Some(position) = self.get_open_position() {
            if matches!(position.state(), State::Closed(_)) {
                self.state.amount += position.close_asset_in_usd() + position.pnl().0;
                self.state.latest_open_position_id = None;
                self.state.trade_positions.remove(&position.id());
                self.statistics.pnl += position.pnl().0;
                self.state.last_trade_time = None;
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

    async fn order_price(
        &self,
        current_price: Decimal,
        order_price: Option<Decimal>,
        is_buy: bool,
    ) -> Result<Decimal, ()> {
        let market_data = self.state.market_data.read().await;
        match order_price {
            Some(v) => Ok(v),
            None => match self.config.atr_spread {
                Some(atr_spread) => {
                    let spread = market_data.atr_by_term(&self.config.atr_term) * atr_spread;
                    if is_buy {
                        Ok(current_price - spread)
                    } else {
                        Ok(current_price + spread)
                    }
                }
                None => Ok(current_price),
            },
        }
    }

    async fn take_profit_distance(&self, current_price: Decimal) -> Option<Decimal> {
        let market_data = self.state.market_data.read().await;
        match self.config.take_profit_ratio {
            Some(v) => Some(v * current_price),
            None => {
                let atr = market_data.atr().1;
                if atr == Decimal::ZERO {
                    None
                } else {
                    Some(atr * self.config.risk_reward)
                }
            }
        }
    }

    async fn target_price(
        &self,
        current_price: Decimal,
        side: OrderSide,
        _is_hedge: bool,
    ) -> Option<Decimal> {
        let take_profit_distance = match self.take_profit_distance(current_price).await {
            Some(v) => v,
            None => return None,
        };

        match self.config.strategy {
            TradingStrategy::RandomWalk(_) | TradingStrategy::MeanReversion(_) => match side {
                OrderSide::Long => Some(current_price + take_profit_distance),
                _ => Some(current_price - take_profit_distance),
            },
        }
    }

    fn take_profit_price(&self, target_price: Decimal) -> Option<Decimal> {
        Some(target_price)
    }

    async fn cut_loss_price(&self, filled_price: Decimal, side: OrderSide) -> Option<Decimal> {
        let market_data = self.state.market_data.read().await;
        let atr = market_data.atr_by_term(&self.config.atr_term);
        let cut_loss_distance = if atr == Decimal::ZERO {
            return None;
        } else {
            atr
        };
        match side {
            OrderSide::Long => Some(filled_price - cut_loss_distance),
            _ => Some(filled_price + cut_loss_distance),
        }
    }

    pub fn clean_canceled_position(&mut self) {
        self.state
            .trade_positions
            .retain(|_, position| !position.is_cancel_expired());
    }

    pub async fn cancel_order(&mut self, order_id: &str, is_already_rejected: bool) {
        if !is_already_rejected {
            if let Err(e) = self
                .state
                .dex_connector
                .cancel_order(&self.config.token_name, order_id)
                .await
            {
                log::error!("cancel_order: {}: order_id = {}", e, order_id);
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

        let position = self.state.trade_positions.get_mut(&position.id()).unwrap();

        let cancel_result = match position.cancel() {
            Ok(cancel_result) => cancel_result,
            Err(_) => {
                log::error!("Failed to cancel the position = {:?}", position);
                return;
            }
        };

        match cancel_result {
            debot_position_manager::CancelResult::OpeningCanceled => {
                // Opening --> Canceled
                // Don't remove the position immediately but do lazily as it might have been filled at the samt time
                //self.state.trade_positions.remove(&position_id);
            }
            debot_position_manager::CancelResult::ClosingCanceled => {
                // Closing --> Open
            }
            debot_position_manager::CancelResult::PartiallyFilled => {
                // Opening --> Open
                if self.state.latest_open_position_id.is_none() {
                    self.state.latest_open_position_id = Some(position.id());
                } else {
                    // Ignore the paritally filled position
                    position.ignore();
                    // dito
                    //self.state.trade_positions.remove(&position_id);
                }
            }
        }

        log::info!("cancel_order succeeded: order_id = {}", order_id);
    }

    pub async fn cancel_all_orders(&mut self) {
        let positions_to_cancel: Vec<TradePosition> = self
            .state
            .trade_positions
            .iter()
            .filter(|(_k, v)| matches!(v.state(), State::Opening))
            .map(|(_k, v)| v.clone())
            .collect();

        for position in &positions_to_cancel {
            self.cancel_order(position.order_id(), false).await;
        }
    }

    pub async fn liquidate(&mut self, reason: Option<String>) {
        let market_data = self.state.market_data.read().await;

        for (_, position) in self.state.trade_positions.iter_mut() {
            let _ = position.on_liquidated(
                market_data.last_price(),
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

        self.state.trade_positions.clear();
    }

    pub async fn is_profitable_position(&self, position_id: u32) -> bool {
        match self.state.trade_positions.get(&position_id) {
            Some(position) => {
                let min_profit_ratio = Decimal::new(1, 3);
                let current_price = self.state.market_data.read().await.last_price();
                if position.position_type() == PositionType::Long {
                    current_price
                        > position.average_open_price() * (Decimal::ONE + min_profit_ratio)
                } else {
                    current_price
                        < position.average_open_price() * (Decimal::ONE - min_profit_ratio)
                }
            }
            None => {
                log::warn!("Open position not found: id = {}", position_id);
                false
            }
        }
    }

    pub fn check_positions(&mut self, price: Decimal) {
        for (_, position) in &mut self.state.trade_positions {
            position.update_counter();
            position.print_info(price);
        }
    }

    pub fn reset_dex_client(&mut self, dex_connector: Arc<DexConnectorBox>) {
        self.state.dex_connector = dex_connector;
    }
}
