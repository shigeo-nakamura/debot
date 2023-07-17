// arbitrage_trader.rs

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::dex::Dex;
use crate::token::Token;
use crate::trade::find_index;

use async_trait::async_trait;
use ethers::prelude::{Provider, SignerMiddleware};
use ethers::providers::Http;
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::NonceManagerMiddleware;
use shared_mongodb::ClientHolder;

use super::abstract_trader::{BaseTrader, TraderState};
use super::{AbstractTrader, DBHandler, Operation, TradeOpportunity, TransactionLog};
pub struct ArbitrageTrader {
    base_trader: BaseTrader,
    num_swaps: usize,
}

impl ArbitrageTrader {
    #[allow(dead_code)]
    pub fn new(
        trader_state: TraderState,
        leverage: f64,
        initial_amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        dry_run: bool,
        num_swaps: usize,
        gas: f64,
        db_client: Arc<Mutex<ClientHolder>>,
        transaction_log: Arc<TransactionLog>,
        save_prices: bool,
    ) -> Self {
        let db_handler = Arc::new(Mutex::new(DBHandler::new(
            db_client.clone(),
            transaction_log.clone(),
        )));

        Self {
            base_trader: BaseTrader::new(
                "Arbitrager".to_string(),
                trader_state,
                leverage,
                initial_amount,
                allowance_factor,
                tokens,
                base_token,
                dexes,
                dry_run,
                gas,
                db_handler,
                None,
                save_prices,
            ),
            num_swaps,
        }
    }

    #[allow(dead_code)]
    fn find_arbitrage_paths_recursive(
        tokens: &Arc<Vec<Box<dyn Token>>>,
        dexes: &Arc<Vec<Box<dyn Dex>>>,
        paths: &mut Vec<Vec<(Box<dyn Token>, Box<dyn Dex>)>>,
        base_token: &Arc<Box<dyn Token>>,
        start_token: &Box<dyn Token>,
        visited: &mut HashSet<(String, String)>,
        path: &mut Vec<(Box<dyn Token>, Box<dyn Dex>)>,
        num_swaps: usize,
    ) -> Result<(), Box<dyn Error>> {
        print_path(path);

        if let Some(_first_path) = path.first() {
            if let Some(last_path) = path.last() {
                if path.len() >= 2
                    && path.len() <= num_swaps
                    && last_path.0.symbol_name() == (*base_token).symbol_name()
                {
                    paths.push(path.clone());
                    log::trace!("Found a valid path. Total paths now: {}", paths.len());
                    return Ok(());
                } else if path.len() >= num_swaps {
                    return Ok(());
                }
            }
        }

        for token in tokens.iter() {
            if path.len() == 0 {
                if token.symbol_name() != start_token.symbol_name() {
                    continue;
                }
            }

            for dex in dexes.iter() {
                // skip visited edges
                let token_dex_pair = (String::from(token.symbol_name()), String::from(dex.name()));
                if visited.contains(&token_dex_pair) {
                    continue;
                }

                log::trace!(
                    "Inspecting token: {} with dex: {}",
                    token.symbol_name(),
                    dex.name()
                );

                // get the last token in the path
                let previous_token = path
                    .last()
                    .map(|(token, _)| token.symbol_name())
                    .unwrap_or_else(|| base_token.symbol_name());

                if previous_token == token.symbol_name() {
                    continue;
                }

                // add the edge to the visited set and path
                visited.insert(token_dex_pair.clone());
                path.push((token.clone(), dex.clone()));

                // recursively explore the remaining path
                Self::find_arbitrage_paths_recursive(
                    tokens,
                    dexes,
                    paths,
                    &base_token,
                    &start_token,
                    visited,
                    path,
                    num_swaps,
                )?;

                // remove the edge from the visited set and path
                visited.remove(&token_dex_pair);
                path.pop();
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn find_arbitrage_paths(
        &self,
    ) -> Result<Vec<Vec<(Box<dyn Token>, Box<dyn Dex>)>>, Box<dyn Error>> {
        // Store arbitrage paths
        let mut paths: Vec<Vec<(Box<dyn Token>, Box<dyn Dex>)>> = Vec::new();

        // Find arbitrage paths for each token
        for token in self.tokens().iter() {
            if token.symbol_name() != self.base_token().symbol_name() {
                let mut visited_pairs: HashSet<(String, String)> = HashSet::new();
                let mut path: Vec<(Box<dyn Token>, Box<dyn Dex>)> = Vec::new();
                ArbitrageTrader::find_arbitrage_paths_recursive(
                    &self.tokens(),
                    &self.dexes(),
                    &mut paths,
                    &self.base_token(),
                    token,
                    &mut visited_pairs,
                    &mut path,
                    self.num_swaps,
                )?;
            }
        }

        Ok(paths)
    }

    #[allow(dead_code)]
    fn calculate_arbitrage_profit(
        path: &Vec<(Box<dyn Token>, Box<dyn Dex>)>,
        amount: f64,
        token_pairs_prices: &HashMap<(String, String, String), f64>,
        base_token: &Box<dyn Token>,
        amounts: &mut Vec<f64>,
    ) -> Option<f64> {
        let mut remaining_amount = amount;
        let path_len = path.len();

        for i in 0..path_len {
            let token_a_name = if i == 0 {
                base_token.symbol_name()
            } else {
                path[i - 1].0.symbol_name()
            };
            let token_b_name = path[i].0.symbol_name();
            let dex_name = path[i].1.name();

            let token_price_result = token_pairs_prices.get(&(
                token_a_name.to_owned(),
                token_b_name.to_owned(),
                dex_name.to_owned(),
            ));

            if let None = token_price_result {
                log::debug!(
                    "No price found for pair {}-{}@{}, skipping",
                    token_a_name,
                    token_b_name,
                    dex_name,
                );
                return None;
            }
            let output_amount = remaining_amount * token_price_result.unwrap();
            amounts.push(output_amount);
            remaining_amount = output_amount;
        }

        let profit = remaining_amount - amount;
        Some(profit)
    }

    #[allow(dead_code)]
    pub async fn find_opportunities(
        &self,
        paths: &Vec<Vec<(Box<dyn Token>, Box<dyn Dex>)>>,
    ) -> Result<Vec<TradeOpportunity>, Box<dyn Error + Send + Sync>> {
        // Get the prices of all token pairs
        let token_pair_prices = self.get_token_pair_prices().await?;

        for ((token_a, token_b, dex), price) in &token_pair_prices {
            log::debug!(
                "Token pair price: {}-{}@{}: {}",
                token_a,
                token_b,
                dex,
                price
            );
        }

        // Calculate arbitrage profits for each path
        let mut results: Vec<TradeOpportunity> = vec![];

        for path in paths {
            let mut amounts = vec![];

            let profit = ArbitrageTrader::calculate_arbitrage_profit(
                path,
                self.initial_amount() * self.leverage(),
                &token_pair_prices,
                &self.base_trader.base_token(),
                &mut amounts,
            );
            if profit.is_none() {
                continue;
            }

            let mut dex_index = Vec::new();
            for dex_in_path in path.iter() {
                let dex_idx = find_index(&**self.dexes(), |dex| dex.name() == dex_in_path.1.name())
                    .expect("dex not found in dexes");
                dex_index.push(dex_idx);
            }

            let mut token_index = Vec::new();
            for token_in_path in path.iter() {
                let token_idx = find_index(&**self.tokens(), |token| {
                    token.symbol_name() == token_in_path.0.symbol_name()
                })
                .expect("token not found in tokens");
                token_index.push(token_idx);
            }

            let gas = self.base_trader.gas() * (path.len() as f64);
            let profit = profit.unwrap() - gas;

            let opportunity = TradeOpportunity {
                dex_index,
                token_index,
                amounts,
                operation: Operation::Buy,
                predicted_profit: Some(profit),
                currect_price: None,
                predicted_price: None,
                trader_name: self.name().to_owned(),
                reason_for_sell: None,
                atr: None,
            };

            results.push(opportunity);
        }
        Ok(results)
    }
}

#[async_trait]
impl AbstractTrader for ArbitrageTrader {
    async fn execute_transactions(
        &mut self,
        _opportunities: &Vec<TradeOpportunity>,
        _wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        _address: Address,
        _deadline_secs: u64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        todo!("Not implemented");
    }

    async fn get_token_pair_prices(
        &self,
    ) -> Result<HashMap<(String, String, String), f64>, Box<dyn Error + Send + Sync>> {
        // Get the prices of all token pairs
        let mut get_price_futures = Vec::new();
        let base_token = &self.base_token();
        let dexes = &self.dexes();
        let tokens = &self.tokens();
        let amount = self.initial_amount() * self.leverage();

        // Get prices with base token / each token and each token / base token
        for dex in dexes.iter() {
            let mut dex_get_price_futures = self
                .base_trader
                .get_token_pair_prices(dex, base_token, tokens, amount)
                .await;
            get_price_futures.append(&mut dex_get_price_futures);
        }

        // Wait for all token price futures to finish
        let prices_results: Vec<
            Result<
                Result<Option<(String, String, String, f64)>, Box<dyn Error + Send + Sync>>,
                tokio::task::JoinError,
            >,
        > = futures::future::join_all(get_price_futures).await;

        let mut token_pair_prices = HashMap::new();

        // Aggregate all the prices into a HashMap
        for price_result in prices_results.into_iter() {
            if let Ok(Ok(Some((token_1_symbol, token_2_symbol, dex_name, price)))) = price_result {
                token_pair_prices.insert((token_1_symbol, token_2_symbol, dex_name), price);
            }
        }

        // Calculate cross prices
        for dex_arc in dexes.iter() {
            let dex_name = dex_arc.name().to_owned();
            for token_a_arc in tokens.iter() {
                for token_b_arc in tokens.iter() {
                    if token_b_arc.symbol_name() != self.base_token().symbol_name() {
                        continue;
                    }
                    if token_a_arc.symbol_name() == token_b_arc.symbol_name() {
                        continue;
                    }
                    for token_c_arc in tokens.iter() {
                        if token_a_arc.symbol_name() == token_c_arc.symbol_name()
                            || token_b_arc.symbol_name() == token_c_arc.symbol_name()
                        {
                            continue;
                        }
                        if let Some(price_ab) = token_pair_prices.get(&(
                            token_a_arc.symbol_name().to_owned(),
                            token_b_arc.symbol_name().to_owned(),
                            dex_name.clone(),
                        )) {
                            if let Some(price_bc) = token_pair_prices.get(&(
                                token_b_arc.symbol_name().to_owned(),
                                token_c_arc.symbol_name().to_owned(),
                                dex_name.clone(),
                            )) {
                                log::trace!(
                                    "cross price: {}-{}@{}",
                                    token_a_arc.symbol_name(),
                                    token_c_arc.symbol_name(),
                                    dex_name
                                );
                                token_pair_prices.insert(
                                    (
                                        token_a_arc.symbol_name().to_owned(),
                                        token_c_arc.symbol_name().to_owned(),
                                        dex_name.clone(),
                                    ),
                                    price_ab * price_bc,
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(token_pair_prices)
    }

    fn state(&self) -> TraderState {
        self.base_trader.state()
    }

    async fn set_state(&mut self, state: TraderState) {
        self.base_trader.set_state(state).await;
    }

    fn leverage(&self) -> f64 {
        self.base_trader.leverage()
    }

    fn initial_amount(&self) -> f64 {
        self.base_trader.initial_amount()
    }

    fn tokens(&self) -> Arc<Vec<Box<dyn Token>>> {
        self.base_trader.tokens()
    }

    fn base_token(&self) -> Arc<Box<dyn Token>> {
        self.base_trader.base_token()
    }

    fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>> {
        self.base_trader.dexes()
    }

    fn name(&self) -> &str {
        self.base_trader.name()
    }

    fn db_handler(&self) -> &Arc<Mutex<DBHandler>> {
        self.base_trader.db_handler()
    }

    async fn init(
        &mut self,
        owner: Address,
        min_managed_amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_trader.init(owner, min_managed_amount).await
    }

    async fn transfer_token(
        &self,
        recipient: Address,
        token: &Box<dyn Token>,
        amount: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_trader
            .transfer_token(recipient, token, amount)
            .await
    }

    async fn calculate_and_log_balance(
        &mut self,
        chain_name: &str,
        wallet_address: &Address,
    ) -> Option<f64> {
        self.base_trader
            .calculate_and_log_balance(chain_name, wallet_address)
            .await
    }
}

fn print_path(path: &Vec<(Box<dyn Token>, Box<dyn Dex>)>) {
    let path_string: Vec<String> = path
        .iter()
        .map(|(token, dex)| format!("Token: {}, Dex: {}", token.symbol_name(), dex.name(),))
        .collect();

    log::trace!(
        "Current path({}): {:?}",
        path.len(),
        path_string.join(" -> ")
    );
}
