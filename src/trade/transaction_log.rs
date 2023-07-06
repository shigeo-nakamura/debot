use mongodb::Database;
use serde::{Deserialize, Serialize};
use shared_mongodb::{database, ClientHolder};
use std::sync::Arc;
use std::time::SystemTime;
use std::{alloc::System, error};

use crate::db::search_item;
use crate::db::{
    insert_item,
    item::{search_items, update_item},
};

use super::TradePosition;

pub async fn get_last_transaction_id(db: &Database) -> u32 {
    let mut last_counter = 0;
    let item = TradePosition::default();
    match search_items(db, &item).await {
        Ok(mut items) => {
            if items.len() > 0 {
                let last_transaction = items.pop();
                last_counter = last_transaction.unwrap().id.unwrap();
            }
        }
        Err(e) => {
            log::warn!(" get_last_transaction_id: {:?}", e);
        }
    };
    last_counter
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppState {
    pub id: u32,
    pub last_execution_time: SystemTime,
    pub prev_balance: Option<f64>,
}

impl AppState {
    pub fn default() -> Self {
        Self {
            id: 1,
            last_execution_time: SystemTime::UNIX_EPOCH,
            prev_balance: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BalanceLog {
    pub date: String,
    pub change: f64,
}
pub struct TransactionLog {
    max_counter: u32,
    counter: std::sync::Mutex<u32>,
    db_name: String,
    prev_balance: Option<f64>,
}

impl TransactionLog {
    pub fn new(max_counter: u32, counter: u32, db_name: &str) -> Self {
        TransactionLog {
            max_counter,
            counter: std::sync::Mutex::new(counter),
            db_name: db_name.to_owned(),
            prev_balance: None,
        }
    }

    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    pub fn increment_counter(&self) -> u32 {
        let mut counter = self.counter.lock().unwrap();
        *counter += 1;
        let mut transaction_id = *counter % (self.max_counter + 1);
        if transaction_id == 0 {
            transaction_id = 1;
        }
        *counter = transaction_id;
        drop(counter);
        transaction_id
    }

    pub async fn get_db(
        &self,
        db_client: &Arc<tokio::sync::Mutex<ClientHolder>>,
    ) -> Option<Database> {
        let db = match database::get(db_client, self.db_name()).await {
            Ok(db) => Some(db),
            Err(e) => {
                log::error!("get_db: {:?}", e);
                None
            }
        };
        db
    }

    pub async fn get_all_open_positions(db: &Database) -> Vec<TradePosition> {
        let item = TradePosition::default();
        let items = match search_items(db, &item).await {
            Ok(items) => items
                .into_iter()
                .filter(|position| position.sold_amount == None)
                .collect(),
            Err(e) => {
                log::error!("get_all_open_positions: {:?}", e);
                vec![]
            }
        };
        log::trace!("{:?}", items);
        items
    }

    pub async fn update_transaction(
        db: &Database,
        item: &TradePosition,
    ) -> Result<(), Box<dyn error::Error>> {
        log::trace!("update: {:?}", item);
        update_item(db, item).await?;
        Ok(())
    }

    pub async fn insert_balance(db: &Database, change: f64) -> Result<(), Box<dyn error::Error>> {
        let current_time = chrono::Utc::now().timestamp();
        let naive_datetime =
            chrono::NaiveDateTime::from_timestamp_opt(current_time, 0).expect("Invalid timestamp");
        let date_string = naive_datetime.format("%Y-%m-%d").to_string();

        let mut item = BalanceLog::default();
        item.date = date_string;
        item.change = change;

        insert_item(db, &item).await?;
        Ok(())
    }

    pub async fn get_app_state(db: &Database) -> AppState {
        let item = AppState::default();
        match search_item(db, &item).await {
            Ok(item) => item,
            Err(e) => {
                log::error!("get_app_state: {:?}", e);
                item
            }
        }
    }

    pub async fn update_app_state(
        db: &Database,
        last_execution_time: SystemTime,
        prev_balance: Option<f64>,
    ) -> Result<(), Box<dyn error::Error>> {
        let mut item = AppState::default();
        item.last_execution_time = last_execution_time;
        item.prev_balance = prev_balance;
        update_item(db, &item).await?;
        Ok(())
    }
}
