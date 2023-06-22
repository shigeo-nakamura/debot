use chrono::TimeZone;
use chrono::{DateTime, Utc};
use chrono_tz::Europe::Berlin;
use chrono_tz::Tz;
use mongodb::Database;
use std::error;
use std::sync::Mutex;

use crate::db::item::{search_items, update_item};
use crate::db::TransactionLogItem;

fn get_nowtime() -> DateTime<Tz> {
    let utc = Utc::now().naive_utc();
    return Berlin.from_utc_datetime(&utc);
}

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
    counter: Mutex<u32>,
    db_name: String,
}

impl TransactionLog {
    pub fn new(max_counter: u32, counter: u32, db_name: &str) -> Self {
        TransactionLog {
            max_counter: max_counter,
            counter: Mutex::new(counter),
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

    pub async fn search(db: &Database, item: &TransactionLogItem) -> Vec<TransactionLogItem> {
        let items = match search_items(db, item).await {
            Ok(items) => items,
            Err(e) => {
                vec![]
            }
        };
        items
    }

    pub async fn update(
        db: &Database,
        item: &TransactionLogItem,
    ) -> Result<(), Box<dyn error::Error>> {
        let dt = get_nowtime();
        log::trace!("update: {:?}, id = {}", item, item.id);
        update_item(db, item).await?;
        Ok(())
    }
}
