use mongodb::Database;
use serde::{Deserialize, Serialize};
use shared_mongodb::{database, ClientHolder};
use std::error;
use std::sync::Arc;
use std::time::SystemTime;

use crate::db::search_item;
use crate::db::{
    insert_item,
    item::{search_items, update_item},
};
use crate::utils::{DateTimeUtils, ToDateTimeString};

use super::TradePosition;

pub enum CounterType {
    Transaction,
    Price,
}

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
    pub last_execution_time: Option<SystemTime>,
    pub prev_balance: Option<f64>,
    pub liquidated_time: Vec<String>,
}

impl AppState {
    pub fn default() -> Self {
        Self {
            id: 1,
            last_execution_time: None,
            prev_balance: None,
            liquidated_time: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BalanceLog {
    pub date: String,
    pub change: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PriceLog {
    pub id: u32,
    weth: f64,
    wbtc: f64,
    wmatic: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PerformanceLog {
    pub id: u32,
    pub system_time: SystemTime,
    pub date: String,
    pub scores: Vec<(String, f64)>,
}

impl PerformanceLog {
    pub fn default() -> Self {
        let now = SystemTime::now();
        Self {
            id: 0,
            system_time: now,
            date: now.to_datetime_string(),
            scores: vec![],
        }
    }
}

pub struct TransactionLog {
    max_counter: u32,
    transaction_counter: std::sync::Mutex<u32>,
    price_counter: std::sync::Mutex<u32>,
    db_name: String,
}

impl TransactionLog {
    pub fn new(
        max_counter: u32,
        transaction_counter: u32,
        price_counter: u32,
        db_name: &str,
    ) -> Self {
        TransactionLog {
            max_counter,
            transaction_counter: std::sync::Mutex::new(transaction_counter),
            price_counter: std::sync::Mutex::new(price_counter),
            db_name: db_name.to_owned(),
        }
    }

    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    pub fn increment_counter(&self, counter_type: CounterType) -> u32 {
        let counter = match counter_type {
            CounterType::Transaction => &self.transaction_counter,
            CounterType::Price => &self.price_counter,
        };

        let mut counter = counter.lock().unwrap();
        *counter += 1;
        let mut id = *counter % (self.max_counter + 1);
        if id == 0 {
            id = 1;
        }
        *counter = id;
        drop(counter);
        id
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
        log::trace!("get_all_open_position: {:?}", items);
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
        let mut item = BalanceLog::default();
        item.date = DateTimeUtils::get_current_date_string();
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
        last_execution_time: Option<SystemTime>,
        prev_balance: Option<f64>,
        is_liquidated: bool,
    ) -> Result<(), Box<dyn error::Error>> {
        let item = AppState::default();
        let mut item = search_item(db, &item).await?;

        if last_execution_time.is_some() {
            item.last_execution_time = last_execution_time;
        }

        if prev_balance.is_some() {
            item.prev_balance = prev_balance;
        }

        if is_liquidated {
            let date_string = DateTimeUtils::get_current_datetime_string();
            item.liquidated_time.push(date_string);
        }

        update_item(db, &item).await?;
        Ok(())
    }
}
