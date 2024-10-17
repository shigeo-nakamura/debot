// db_operations.rs

use debot_db::{
    CandlePattern, CounterType, DebugLog, ModelParams, PnlLog, PositionLog, PriceLog, PricePoint,
    TransactionLog,
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
        max_position_counter: Option<u32>,
        max_price_counter: Option<u32>,
        max_balance_counter: Option<u32>,
        mongodb_uri: &str,
        db_w_name: &str,
        db_r_name: &str,
        back_test: bool,
        path_to_models: Option<&String>,
    ) -> Self {
        let transaction_log = Arc::new(
            TransactionLog::new(
                max_position_counter,
                max_price_counter,
                max_balance_counter,
                mongodb_uri,
                db_r_name,
                db_w_name,
                back_test,
            )
            .await,
        );

        let model_params = ModelParams::new(
            &mongodb_uri,
            &db_r_name,
            path_to_models.is_none(),
            path_to_models.cloned(),
        )
        .await;
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

        if let Some(db) = self.transaction_log.get_w_db().await {
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
        invested_amount: Decimal,
    ) {
        log::info!("log_app_state: {:?}", last_execution_time);

        if let Some(db) = self.transaction_log.get_w_db().await {
            if let Err(e) = TransactionLog::update_app_state(
                &db,
                last_execution_time,
                last_equity,
                circuit_break,
                error_time,
                invested_amount,
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

        if !matches!(position.state(), State::Closed(_)) {
            return;
        }

        if let Some(db) = self.transaction_log.get_w_db().await {
            log::debug!("candle_pattern = {:?}", position.candle_pattern());

            let valid_data = || match position.state() {
                State::Closed(reason) => match reason.as_str() {
                    "TakeProfit" | "CutLoss" | "Expired" => true,
                    _ => false,
                },
                _ => false,
            };

            let position_log = PositionLog {
                id: Some(position.id()),
                fund_name: position.fund_name().to_owned(),
                order_id: position.order_id().to_owned(),
                ordered_price: position.ordered_price(),
                state: position.state().to_string(),
                token_name: position.token_name().to_owned(),
                open_time_str: position.open_time_str().to_owned(),
                open_timestamp: position.open_timestamp(),
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
                pnl: position.pnl().0,
                fee: position.fee(),
                debug: DebugLog {
                    input_1: position.price().3.round_dp(4),
                    input_2: position.price().4.round_dp(4),
                    input_3: position.price().5.round_dp(4),
                    input_4: position.atr().1.round_dp(4),
                    input_5: position.atr().2.round_dp(4),
                    input_6: position.atr().3.round_dp(4),
                    input_7: position.atr().4.round_dp(4),
                    input_8: position.atr().5.round_dp(4),
                    input_9: position.rsi().1.round_dp(4),
                    input_10: position.rsi().2.round_dp(4),
                    input_11: position.rsi().3.round_dp(4),
                    input_12: position.rsi().4.round_dp(4),
                    input_13: position.rsi().5.round_dp(4),
                    input_14: position.last_volume().unwrap_or_default().round_dp(4),
                    input_15: position.last_num_trades().unwrap_or_default().into(),
                    input_16: position.last_funding_rate().unwrap_or_default().round_dp(4),
                    input_17: position
                        .last_open_interest()
                        .unwrap_or_default()
                        .round_dp(4),
                    input_18: position.last_oracle_price().unwrap_or_default().round_dp(4)
                        - position.price().0.round_dp(4),
                    input_19: Decimal::ZERO,
                    input_20: position.atr_spread().round_dp(4),
                    input_21: position.open_tick_count_max().into(),
                    input_22: if position.take_profit_ratio().is_zero() {
                        Decimal::ZERO
                    } else {
                        Decimal::ONE
                    },
                    input_23: position.atr_term().round_dp(4),
                    input_24: Decimal::ZERO,
                    input_25: Decimal::ZERO,
                    input_26: Decimal::ZERO,
                    input_27: Decimal::ZERO,
                    input_28: Decimal::ZERO,
                    input_29: Decimal::ZERO,
                    input_30: position.candle_pattern().0,
                    input_31: position.candle_pattern().1,
                    input_32: position.candle_pattern().2,
                    input_33: position.candle_pattern().3,
                    input_34: CandlePattern::None,
                    input_35: CandlePattern::None,
                    input_36: CandlePattern::None,
                    input_37: CandlePattern::None,
                    input_38: CandlePattern::None,
                    input_39: CandlePattern::None,
                    output_1: if valid_data() {
                        if position.pnl().0 > Decimal::ZERO {
                            Decimal::ONE
                        } else {
                            Decimal::ZERO
                        }
                    } else {
                        Decimal::ZERO
                    },
                    output_2: if valid_data() {
                        if position.pnl().1 > Decimal::ZERO {
                            position.pnl().1.round_dp(4)
                        } else {
                            Decimal::ZERO
                        }
                    } else {
                        Decimal::ZERO
                    },
                    output_3: if position.pnl().0 > Decimal::ZERO {
                        Some(position.tick_to_fill().into())
                    } else {
                        Some(Decimal::new(-1, 0))
                    },
                    output_4: None,
                    output_5: None,
                },
            };

            if let Err(e) = TransactionLog::update_transaction(&db, &position_log).await {
                log::error!("log_position: {:?}", e);
            }
        }
    }

    pub async fn log_price(&self, name: &str, token_name: &str, price_point: PricePoint) {
        if let Some(db) = self.transaction_log.get_w_db().await {
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
        if let Some(db) = self.transaction_log.get_w_db().await {
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

    pub async fn get_latest_price_market_data(
        &self,
        limit: Option<u32>,
    ) -> HashMap<String, HashMap<String, Vec<PricePoint>>> {
        if let Some(db) = self.transaction_log.get_r_db().await {
            let mut data = TransactionLog::get_price_market_data(&db, None, None, true).await;

            if let Some(data_size) = limit {
                for (_, token_map) in data.iter_mut() {
                    for (_, price_points) in token_map.iter_mut() {
                        if price_points.len() > data_size as usize {
                            let start_index = price_points.len() - data_size as usize;
                            *price_points = price_points[start_index..].to_vec();
                        }
                    }
                }
            }

            data
        } else {
            HashMap::new()
        }
    }

    pub async fn create_random_forest(&self, key: &str) -> RandomForest {
        RandomForest::new(key, &self.model_params).await
    }
}
