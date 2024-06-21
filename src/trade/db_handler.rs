// db_operations.rs

use debot_db::{
    CounterType, DebugLog, ModelParams, PnlLog, PositionLog, PriceLog, PricePoint, TransactionLog,
};
use debot_ml::RandomForest;
use debot_position_manager::{PositionType, State, TradePosition};
use debot_utils::DateTimeUtils;
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::{collections::HashMap, env, sync::Arc, time::SystemTime};

pub struct DBHandler {
    transaction_log: Arc<TransactionLog>,
    model_params: Arc<ModelParams>,
}

lazy_static! {
    static ref SAVE_POSITION: bool = {
        match env::var("SAVE_POSITION") {
            Ok(val) => val.parse::<bool>().unwrap_or(true),
            Err(_) => true,
        }
    };
}

impl DBHandler {
    pub async fn new(
        max_position_counter: u32,
        max_price_counter: u32,
        max_balance_counter: u32,
        mongodb_uri: &str,
        db_name: &str,
    ) -> Self {
        let transaction_log = Arc::new(
            TransactionLog::new(
                max_position_counter,
                max_price_counter,
                max_balance_counter,
                mongodb_uri,
                db_name,
            )
            .await,
        );

        let model_params = ModelParams::new(&mongodb_uri, &db_name).await;
        let model_params = Arc::new(model_params);

        Self {
            transaction_log,
            model_params,
        }
    }
}

impl DBHandler {
    pub async fn log_pnl(&self, pnl: Decimal) {
        log::info!("log_pnl: {:6.6}", pnl);

        if let Some(db) = self.transaction_log.get_db().await {
            let mut item = PnlLog::default();
            item.id = self.increment_counter(CounterType::Pnl);
            item.date = DateTimeUtils::get_current_date_string();
            item.pnl = pnl;

            if let Err(e) = TransactionLog::insert_pnl(&db, item).await {
                log::error!("log_pnl: {:?}", e);
            }
        }
    }

    pub async fn log_app_state(
        &self,
        last_execution_time: Option<SystemTime>,
        last_equity: Option<Decimal>,
        circuit_break: bool,
        error_time: Option<String>,
    ) {
        log::info!("log_app_state: {:?}", last_execution_time);

        if let Some(db) = self.transaction_log.get_db().await {
            if let Err(e) = TransactionLog::update_app_state(
                &db,
                last_execution_time,
                last_equity,
                circuit_break,
                error_time,
            )
            .await
            {
                log::warn!("log_app_state: {:?}", e);
            }
        }
    }

    pub async fn log_position(&self, position: &TradePosition) {
        if *SAVE_POSITION == false {
            return;
        }
        if matches!(
            position.state(),
            State::Opening | State::Closing(_) | State::Canceled(_)
        ) {
            return;
        }

        if let Some(db) = self.transaction_log.get_db().await {
            let position_log = PositionLog {
                id: Some(position.id()),
                fund_name: position.fund_name().to_owned(),
                order_id: position.order_id().to_owned(),
                ordered_price: position.ordered_price(),
                state: position.state().to_string(),
                token_name: position.token_name().to_owned(),
                open_time_str: position.open_time_str().to_owned(),
                close_time_str: position.close_time_str().to_owned(),
                average_open_price: position.average_open_price(),
                position_type: if position.position_type() == PositionType::Long {
                    "Long"
                } else {
                    "Short"
                }
                .to_string(),
                close_price: position.close_price(),
                asset_in_usd: position.asset_in_usd(),
                pnl: position.pnl(),
                fee: position.fee(),
                debug: DebugLog {
                    input_1: position.adx().0.round_dp(4),
                    input_2: position.adx().1.round_dp(4),
                    input_3: position.adx().2.round_dp(4),
                    input_4: position.atr().0.round_dp(4),
                    input_5: position.atr().1.round_dp(4),
                    input_6: position.atr().2.round_dp(4),
                    input_7: position.rsi().0.round_dp(4),
                    input_8: position.rsi().1.round_dp(4),
                    input_9: position.rsi().2.round_dp(4),
                    input_10: position.macd().round_dp(4),
                    input_11: position.stochastic().0.round_dp(4),
                    input_12: position.stochastic().1.round_dp(4),
                    input_13: position.stochastic().2.round_dp(4),
                    input_14: position.take_profit_ratio().round_dp(4),
                    input_15: position.price_variance().round_dp(4),
                    input_16: position.atr_spread().round_dp(4),
                    input_17: position.max_open_duration().into(),
                    input_18: Decimal::ZERO,
                    input_19: Decimal::ZERO,
                    input_20: Decimal::ZERO,
                    output_1: match position.state() {
                        State::Closed(reason) => match reason.as_str() {
                            "TakeProfit" => Decimal::new(1, 0),
                            "CutLoss" => Decimal::new(-1, 0),
                            _ => Decimal::ZERO,
                        },
                        _ => Decimal::ZERO,
                    },
                    output_2: if position.pnl() / position.fee() > Decimal::new(10, 0) {
                        Decimal::ONE
                    } else {
                        Decimal::ZERO
                    },
                },
            };

            if let Err(e) = TransactionLog::update_transaction(&db, &position_log).await {
                log::error!("log_position: {:?}", e);
            }
        }
    }

    pub async fn log_price(&self, name: &str, token_name: &str, price_point: PricePoint) {
        if let Some(db) = self.transaction_log.get_db().await {
            let mut item = PriceLog::default();
            item.id = self.increment_counter(CounterType::Price);
            item.name = name.to_owned();
            item.token_name = token_name.to_owned();
            item.price_point = price_point;
            if let Err(e) = TransactionLog::update_price(&db, item).await {
                log::error!("log_price: {:?}", e);
            }
        }
    }

    pub fn increment_counter(&self, counter_type: CounterType) -> Option<u32> {
        let counter_type = match counter_type {
            CounterType::Position => debot_db::CounterType::Position,
            CounterType::Price => debot_db::CounterType::Price,
            CounterType::Pnl => debot_db::CounterType::Pnl,
        };
        Some(self.transaction_log.increment_counter(counter_type))
    }

    pub async fn get_app_state(&self) -> (Option<SystemTime>, Option<Decimal>, bool) {
        if let Some(db) = self.transaction_log.get_db().await {
            let app_state = TransactionLog::get_app_state(&db).await;
            (
                app_state.last_execution_time,
                app_state.last_equity,
                app_state.curcuit_break,
            )
        } else {
            (None, None, true)
        }
    }

    pub async fn get_price_market_data(&self) -> HashMap<String, HashMap<String, Vec<PricePoint>>> {
        if let Some(db) = self.transaction_log.get_db().await {
            TransactionLog::get_price_market_data(&db).await
        } else {
            HashMap::new()
        }
    }

    pub async fn create_random_forest(&self, key: &str) -> RandomForest {
        RandomForest::new(key, &self.model_params).await
    }
}
