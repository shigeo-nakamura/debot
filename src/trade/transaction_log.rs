// transaction_log.rs

use crate::db::{
    insert_item,
    item::{search_items, update_item},
    Entity,
};
use crate::db::{search_item, Counter, CounterType};
use debot_market_analyzer::PricePoint;
use debot_position_manager::TradePosition;
use debot_utils::{DateTimeUtils, HasId};
use mongodb::Database;
use serde::{Deserialize, Serialize};
use shared_mongodb::{database, ClientHolder};
use std::collections::HashMap;
use std::error;
use std::sync::Arc;
use std::time::SystemTime;

async fn get_last_id<T: Default + Entity + HasId>(db: &Database) -> u32 {
    let item = T::default();
    match search_items(db, &item).await {
        Ok(mut items) => items.pop().and_then(|item| item.id()).unwrap_or(0),
        Err(e) => {
            log::warn!("get_last_id: {:?}", e);
            0
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppState {
    pub id: u32,
    pub last_execution_time: Option<SystemTime>,
    pub liquidated_time: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            id: 1,
            last_execution_time: None,
            liquidated_time: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PnlLog {
    pub id: Option<u32>,
    pub date: String,
    pub pnl: f64,
}

impl HasId for PnlLog {
    fn id(&self) -> Option<u32> {
        self.id
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PriceLog {
    pub id: Option<u32>,
    pub name: String,
    pub token_name: String,
    pub price_point: PricePoint,
}

impl HasId for PriceLog {
    fn id(&self) -> Option<u32> {
        self.id
    }
}

pub struct TransactionLog {
    counter: Counter,
    db_name: String,
}

impl TransactionLog {
    pub fn new(
        max_position_counter: u32,
        max_price_couner: u32,
        max_balance_counter: u32,
        position_counter: u32,
        price_counter: u32,
        balance_counter: u32,
        db_name: &str,
    ) -> Self {
        let counter = Counter::new(
            max_position_counter,
            max_price_couner,
            max_balance_counter,
            position_counter,
            price_counter,
            balance_counter,
        );

        TransactionLog {
            counter,
            db_name: db_name.to_owned(),
        }
    }

    pub fn increment_counter(&self, counter_type: CounterType) -> u32 {
        self.counter.increment(counter_type)
    }

    pub async fn get_last_transaction_id(db: &Database, counter_type: CounterType) -> u32 {
        match counter_type {
            CounterType::Position => get_last_id::<TradePosition>(db).await,
            CounterType::Price => get_last_id::<PriceLog>(db).await,
            CounterType::Pnl => get_last_id::<PnlLog>(db).await,
        }
    }

    pub fn db_name(&self) -> &str {
        &self.db_name
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
                .filter(|position| position.close_amount == None)
                .collect(),
            Err(_) => {
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
        update_item(db, item).await?;
        Ok(())
    }

    pub async fn update_price(db: &Database, item: PriceLog) -> Result<(), Box<dyn error::Error>> {
        update_item(db, &item).await?;
        Ok(())
    }

    pub async fn get_price_market_data(
        db: &Database,
    ) -> HashMap<String, HashMap<String, Vec<PricePoint>>> {
        let item = PriceLog::default();
        let items = match search_items(db, &item).await {
            Ok(items) => items,
            Err(e) => {
                log::warn!("get_price_market_data: {:?}", e);
                return HashMap::new();
            }
        };

        let mut result = HashMap::new();

        for price_log in items {
            result
                .entry(price_log.name)
                .or_insert_with(HashMap::new)
                .entry(price_log.token_name)
                .or_insert_with(Vec::new)
                .push(price_log.price_point);
        }

        for (_, token_map) in &mut result {
            for (_, price_points) in token_map {
                price_points.sort_by_key(|pp| pp.timestamp);
            }
        }

        result
    }

    pub async fn insert_pnl(db: &Database, item: PnlLog) -> Result<(), Box<dyn error::Error>> {
        insert_item(db, &item).await?;
        Ok(())
    }

    pub async fn get_app_state(db: &Database) -> AppState {
        let item = AppState::default();
        match search_item(db, &item).await {
            Ok(item) => item,
            Err(e) => {
                log::warn!("get_app_state: {:?}", e);
                item
            }
        }
    }

    pub async fn update_app_state(
        db: &Database,
        last_execution_time: Option<SystemTime>,
        is_liquidated: bool,
    ) -> Result<(), Box<dyn error::Error>> {
        let item = AppState::default();
        let mut item = match search_item(db, &item).await {
            Ok(prev_item) => prev_item,
            Err(_) => item,
        };

        if last_execution_time.is_some() {
            item.last_execution_time = last_execution_time;
        }

        if is_liquidated {
            let date_string = DateTimeUtils::get_current_datetime_string();
            item.liquidated_time.push(date_string);
        }

        update_item(db, &item).await?;
        Ok(())
    }

    pub async fn update_liquidate_time(db: &Database) -> Result<(), Box<dyn error::Error>> {
        TransactionLog::update_app_state(&db, None, true).await
    }
}
