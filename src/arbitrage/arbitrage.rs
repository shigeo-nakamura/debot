// arbitrage.rs

use crate::{dex::Dex, http::PriceData};
use async_trait::async_trait;
use ethers::prelude::LocalWallet;
use std::{
    error::Error,
    sync::{Arc, RwLock},
    time::SystemTime,
};

pub struct ArbitrageOpportunity {
    pub dex1_index: usize,
    pub dex2_index: usize,
    pub token_a_index: usize,
    pub token_b_index: usize,
    pub profit: f64,
    pub amount: f64,
}

#[async_trait]
pub trait Arbitrage {
    async fn find_opportunities(
        &self,
        dexes: &Arc<Vec<(String, Box<dyn Dex>)>>,
        price_history: Arc<RwLock<Vec<PriceData>>>,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>>;

    async fn execute_transactions(
        &self,
        opportunity: &ArbitrageOpportunity,
        dexes: &Arc<Vec<(String, Box<dyn Dex>)>>,
        signer: &LocalWallet,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn store_price_history(
        dex_names: &[&str],
        token_symbols: &[&str],
        prices: &[f64],
        profit: f64,
        price_history: Arc<RwLock<Vec<PriceData>>>,
    ) {
        let mut price_history_guard = price_history.write().unwrap();
        price_history_guard.push(PriceData {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            tokens: token_symbols
                .iter()
                .map(|symbol| String::from(*symbol))
                .collect::<Vec<String>>(),
            dex_prices: dex_names
                .iter()
                .zip(prices.iter())
                .map(|(dex_name, price)| (String::from(*dex_name), *price))
                .collect(),
            profit: profit,
        });
    }
}
