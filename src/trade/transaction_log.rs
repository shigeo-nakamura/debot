// transaction_log.rs

use super::{price_history::PricePoint, TradePosition, TraderState};
use crate::db::{
    insert_item,
    item::{search_items, update_item},
    Entity,
};
use crate::db::{search_item, Counter, CounterType};
use crate::utils::{DateTimeUtils, ToDateTimeString};
use mongodb::Database;
use serde::{Deserialize, Serialize};
use shared_mongodb::{database, ClientHolder};
use std::collections::HashMap;
use std::error;
use std::sync::Arc;
use std::time::SystemTime;

pub trait HasId {
    fn id(&self) -> Option<u32>;
}

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
    pub last_execution_time: HashMap<String, Option<SystemTime>>,
    pub prev_balance: HashMap<String, Option<f64>>,
    pub liquidated_time: Vec<String>,
    pub trader_state: HashMap<String, TraderState>,
    pub latest_scores: HashMap<String, HashMap<String, f64>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            id: 1,
            last_execution_time: HashMap::new(),
            prev_balance: HashMap::new(),
            liquidated_time: vec![],
            trader_state: HashMap::new(),
            latest_scores: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BalanceLog {
    pub id: Option<u32>,
    pub chain_name: String,
    pub date: String,
    pub change: f64,
}

impl HasId for BalanceLog {
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PerformanceLog {
    pub id: Option<u32>,
    pub date: String,
    pub trader_name: String,
    pub scores: HashMap<String, f64>,
}

impl Default for PerformanceLog {
    fn default() -> Self {
        let now = SystemTime::now();
        Self {
            id: None,
            date: now.to_datetime_string(),
            trader_name: String::new(),
            scores: HashMap::new(),
        }
    }
}

impl HasId for PerformanceLog {
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
        max_performance_counter: u32,
        max_balance_counter: u32,
        position_counter: u32,
        price_counter: u32,
        performance_counter: u32,
        balance_counter: u32,
        db_name: &str,
    ) -> Self {
        let counter = Counter::new(
            max_position_counter,
            max_price_couner,
            max_performance_counter,
            max_balance_counter,
            position_counter,
            price_counter,
            performance_counter,
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
            CounterType::Performance => get_last_id::<PerformanceLog>(db).await,
            CounterType::Balance => get_last_id::<BalanceLog>(db).await,
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
                .filter(|position| position.sold_amount == None)
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

    pub async fn update_performance(
        db: &Database,
        item: PerformanceLog,
    ) -> Result<(), Box<dyn error::Error>> {
        update_item(db, &item).await?;
        Ok(())
    }

    pub async fn update_price(db: &Database, item: PriceLog) -> Result<(), Box<dyn error::Error>> {
        update_item(db, &item).await?;
        Ok(())
    }

    pub async fn get_price_histories(
        db: &Database,
    ) -> HashMap<String, HashMap<String, Vec<PricePoint>>> {
        let item = PriceLog::default();
        let items = match search_items(db, &item).await {
            Ok(items) => items,
            Err(e) => {
                log::warn!("get_price_histories: {:?}", e);
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

    pub async fn insert_balance(
        db: &Database,
        item: BalanceLog,
    ) -> Result<(), Box<dyn error::Error>> {
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
        chain_name: &str,
        prev_balance: Option<f64>,
        is_liquidated: bool,
        trader_state: Option<(String, TraderState)>,
        scores: Option<(String, HashMap<String, f64>)>,
    ) -> Result<(), Box<dyn error::Error>> {
        let item = AppState::default();
        let mut item = match search_item(db, &item).await {
            Ok(prev_item) => prev_item,
            Err(_) => item,
        };

        if last_execution_time.is_some() {
            item.last_execution_time
                .insert(chain_name.to_owned(), last_execution_time);
        }

        if prev_balance.is_some() {
            item.prev_balance
                .insert(chain_name.to_owned(), prev_balance);
        }

        if is_liquidated {
            let date_string = DateTimeUtils::get_current_datetime_string();
            item.liquidated_time.push(date_string);
        }

        if let Some((name, state)) = trader_state {
            item.trader_state.insert(name, state);
        }

        if let Some((trader_name, scores)) = scores {
            item.latest_scores.insert(trader_name, scores);
        }

        update_item(db, &item).await?;
        Ok(())
    }

    pub async fn update_liquidate_time(
        db: &Database,
        chain_name: &str,
    ) -> Result<(), Box<dyn error::Error>> {
        TransactionLog::update_app_state(&db, None, chain_name, None, true, None, None).await
    }

    pub async fn update_trader_state(
        db: &Database,
        name: String,
        state: TraderState,
    ) -> Result<(), Box<dyn error::Error>> {
        TransactionLog::update_app_state(
            &db,
            None,
            &String::new(),
            None,
            true,
            Some((name, state)),
            None,
        )
        .await
    }
}
