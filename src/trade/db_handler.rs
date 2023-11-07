// db_operations.rs

use crate::{
    db::CounterType,
    trade::{PnlLog, TransactionLog},
};

use debot_market_analyzer::PricePoint;
use debot_position_manager::TradePosition;
use debot_utils::DateTimeUtils;
use shared_mongodb::ClientHolder;
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::Mutex;

use super::transaction_log::PriceLog;

pub struct DBHandler {
    client_holder: Arc<Mutex<ClientHolder>>,
    transaction_log: Arc<TransactionLog>,
}

impl DBHandler {
    pub fn new(
        client_holder: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
    ) -> Self {
        Self {
            client_holder,
            transaction_log,
        }
    }
}

impl DBHandler {
    pub async fn log_pnl(&self, pnl: f64) {
        log::info!("log_pnl: {:6.6}", pnl);

        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
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
        is_liquidated: bool,
    ) {
        log::info!("log_app_state: {:?}", last_execution_time);

        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
            if let Err(e) =
                TransactionLog::update_app_state(&db, last_execution_time, is_liquidated).await
            {
                log::warn!("log_app_state: {:?}", e);
            }
        }
    }

    pub async fn log_position(&self, position: &TradePosition) {
        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
            if let Err(e) = TransactionLog::update_transaction(&db, position).await {
                log::error!("log_position: {:?}", e);
            }
        }
    }

    pub async fn log_price(&self, name: &str, token_name: &str, price_point: PricePoint) {
        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
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
        Some(self.transaction_log.increment_counter(counter_type))
    }

    pub async fn get_open_positions_map(
        transaction_log: Arc<TransactionLog>,
        client_holder: Arc<Mutex<ClientHolder>>,
    ) -> HashMap<String, Vec<TradePosition>> {
        let mut open_positions_map = HashMap::new();
        if let Some(db) = transaction_log.get_db(&client_holder).await {
            let open_positions_vec = TransactionLog::get_all_open_positions(&db).await;

            // Populate the open_positions_map
            for position in open_positions_vec {
                // Ensure a Vec exists for this fund_name, then push the position into it
                open_positions_map
                    .entry(position.fund_name().to_owned())
                    .or_insert_with(Vec::new)
                    .push(position);
            }

            for (fund_name, positions) in &open_positions_map {
                log::info!("Fund name: {}", fund_name);
                for position in positions {
                    log::info!("Token name: {}", position.token_name());
                    log::info!("Position: {:?}", position);
                }
            }
        }

        open_positions_map
    }
}
