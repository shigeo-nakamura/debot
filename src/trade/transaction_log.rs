use mongodb::Database;
use shared_mongodb::{database, ClientHolder};
use std::error;
use std::sync::Arc;

use crate::db::item::{search_item, update_item};
use crate::db::{insert_item, search_items, BalanceLogItem, TransactionLogItem};

pub async fn get_last_transaction_id(db: &Database) -> u32 {
    let mut last_counter = 0;
    let item = TransactionLogItem::default();
    match search_items(db, &item).await {
        Ok(mut items) => {
            if items.len() > 0 {
                let last_transaction = items.pop();
                last_counter = last_transaction.unwrap().id;
            }
        }
        Err(e) => {
            log::error!("{:?}", e);
        }
    };
    last_counter
}

pub struct TransactionLog {
    max_counter: u32,
    counter: std::sync::Mutex<u32>,
    db_name: String,
}

impl TransactionLog {
    pub fn new(max_counter: u32, counter: u32, db_name: &str) -> Self {
        TransactionLog {
            max_counter,
            counter: std::sync::Mutex::new(counter),
            db_name: db_name.to_owned(),
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

    pub async fn search(db: &Database, item: &TransactionLogItem) -> Option<TransactionLogItem> {
        match search_item(db, item).await {
            Ok(item) => Some(item),
            Err(e) => {
                log::error!("{:?}", e);
                None
            }
        }
    }

    pub async fn get_db(
        &self,
        db_client: &Arc<tokio::sync::Mutex<ClientHolder>>,
    ) -> Option<Database> {
        let db = match database::get(db_client, self.db_name()).await {
            Ok(db) => Some(db),
            Err(e) => {
                log::info!("{:?}", e);
                None
            }
        };
        db
    }

    pub async fn update_transaction(
        db: &Database,
        item: &TransactionLogItem,
    ) -> Result<(), Box<dyn error::Error>> {
        log::trace!("update: {:?}, id = {}", item, item.id);
        update_item(db, item).await?;
        Ok(())
    }

    pub async fn insert_balance(db: &Database, amount: f64) -> Result<(), Box<dyn error::Error>> {
        let current_time = chrono::Utc::now().timestamp();
        let naive_datetime =
            chrono::NaiveDateTime::from_timestamp_opt(current_time, 0).expect("Invalid timestamp");
        let date_string = naive_datetime.format("%Y-%m-%d").to_string();

        let mut item = BalanceLogItem::default();
        item.date = date_string;
        item.amount = amount;

        insert_item(db, &item).await?;
        Ok(())
    }
}
