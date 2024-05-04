// db_operations.rs

use debot_db::{CounterType, PnlLog, PriceLog, PricePoint, Score, ScoreMap, TransactionLog};
use debot_position_manager::{State, TradePosition};
use debot_utils::DateTimeUtils;
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::{collections::HashMap, env, sync::Arc, time::SystemTime};

pub struct DBHandler {
    transaction_log: Arc<TransactionLog>,
}

lazy_static! {
    static ref SAVE_ALL_POSITION_STATE: bool = {
        match env::var("SAVE_ALL_POSITION_STATE") {
            Ok(val) => val.parse::<bool>().unwrap_or(false),
            Err(_) => false,
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
        Self { transaction_log }
    }
}

impl DBHandler {
    pub async fn log_score(&self, score_map: &HashMap<(String, Decimal), i32>) {
        log::info!("log_score: {:?}", score_map);

        if let Some(db) = self.transaction_log.get_db().await {
            let mut item = ScoreMap::default();
            item.scores = score_map
                .iter()
                .map(|((token_name, atr_ratio), score)| Score {
                    token_name: token_name.to_string(),
                    atr_ratio: *atr_ratio,
                    score: *score,
                })
                .collect();

            if let Err(e) = TransactionLog::update_score_map(&db, item).await {
                log::error!("log_score: {:?}", e);
            }
        }
    }

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
        if *SAVE_ALL_POSITION_STATE == false {
            if matches!(
                position.state(),
                State::Opening | State::Closing(_) | State::Canceled(_)
            ) {
                return;
            }
        }
        if let Some(db) = self.transaction_log.get_db().await {
            if let Err(e) = TransactionLog::update_transaction(&db, position).await {
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

    pub async fn get_score(&self) -> HashMap<(String, Decimal), i32> {
        if let Some(db) = self.transaction_log.get_db().await {
            let score_map = TransactionLog::get_score_map(&db).await;
            let mut dest_score_map: HashMap<(String, Decimal), i32> = HashMap::new();
            for score in score_map.scores.iter() {
                dest_score_map.insert((score.token_name.to_owned(), score.atr_ratio), score.score);
            }
            dest_score_map
        } else {
            HashMap::new()
        }
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

    #[allow(dead_code)]
    pub async fn get_open_positions_map(&self) -> HashMap<String, HashMap<u32, TradePosition>> {
        let mut open_positions_map = HashMap::new();
        if let Some(db) = self.transaction_log.get_db().await {
            let open_positions_vec = TransactionLog::get_all_open_positions(&db).await;

            // Populate the open_positions_map
            for position in open_positions_vec {
                // Ensure a Vec exists for this fund_name, then push the position into it
                open_positions_map
                    .entry(position.fund_name().to_owned())
                    .or_insert_with(HashMap::new)
                    .insert(position.id().unwrap_or_default(), position);
            }

            for (fund_name, positions) in &open_positions_map {
                log::info!("Fund name: {}", fund_name);
                for (_, position) in positions {
                    log::info!("Token name: {}", position.token_name());
                    log::info!("Position: {:?}", position);
                }
            }
        }

        open_positions_map
    }
}
