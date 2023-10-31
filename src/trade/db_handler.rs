// db_operations.rs

use crate::{
    db::CounterType,
    trade::{BalanceLog, TransactionLog},
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
    pub async fn log_liquidate_time(&self, chain_name: &str) {
        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
            match TransactionLog::update_liquidate_time(&db, chain_name).await {
                Ok(_) => {}
                Err(e) => {
                    log::warn!("log_liquidate_time: {:?}", e);
                }
            }
        }
    }

    pub async fn log_current_balance(
        &self,
        chain_name: &str,
        current_balance: f64,
        prev_balance: Option<f64>,
    ) {
        log::info!("log_current_balance: {:6.6}", current_balance);

        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
            let change = match prev_balance {
                Some(balance) => balance - current_balance,
                None => 0.0,
            };

            let mut item = BalanceLog::default();
            item.id = self.increment_counter(CounterType::Balance);
            item.chain_name = chain_name.to_owned();
            item.date = DateTimeUtils::get_current_date_string();
            item.change = change;

            if let Err(e) = TransactionLog::insert_balance(&db, item).await {
                log::error!("log_current_balance: {:?}", e);
            }
        }
    }

    pub async fn log_app_state(
        &self,
        last_execution_time: Option<SystemTime>,
        chain_name: &str,
        prev_balance: Option<f64>,
        is_liquidated: bool,
    ) {
        if let Some(db) = self.transaction_log.get_db(&self.client_holder).await {
            if let Err(e) = TransactionLog::update_app_state(
                &db,
                last_execution_time,
                chain_name,
                prev_balance,
                is_liquidated,
            )
            .await
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
    ) -> HashMap<String, HashMap<String, TradePosition>> {
        let mut open_positions_map = HashMap::new();
        if let Some(db) = transaction_log.get_db(&client_holder).await {
            let open_positions_vec = TransactionLog::get_all_open_positions(&db).await;

            // Populate the open_positions_map
            for mut position in open_positions_vec {
                // Restore the mutex
                position.cut_loss_price =
                    Arc::new(std::sync::Mutex::new(position.initial_cut_loss_price));

                // Ensure a HashMap exists for this fund_name
                open_positions_map
                    .entry(position.fund_name.clone())
                    .or_insert_with(HashMap::new)
                    .insert(position.token_name.clone(), position);
            }

            for (fund_name, positions) in &open_positions_map {
                log::info!("Fund name: {}", fund_name);
                for (token_name, position) in positions {
                    log::info!("Token name: {}", token_name);
                    log::info!("Position: {:?}", position);
                }
            }
        }

        open_positions_map
    }
}
