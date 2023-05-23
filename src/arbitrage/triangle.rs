use std::collections::HashSet;
use std::env::consts::DLL_EXTENSION;
use std::error::Error;
use std::sync::{Arc, RwLock};

use crate::arbitrage::find_index;
use crate::arbitrage::Arbitrage;
use crate::dex::Dex;
use crate::token::Token;

use crate::dex::dex::TokenPair;
use crate::http::TransactionResult;
use async_trait::async_trait;
use ethers::prelude::{Provider, SignerMiddleware};
use ethers::providers::Http;
use ethers::signers::LocalWallet;
use ethers::types::Address;
use ethers_middleware::NonceManagerMiddleware;
use tokio::sync::Mutex;

use super::arbitrage::BaseArbitrage;
use super::ArbitrageOpportunity;
use async_recursion::async_recursion;
pub struct TriangleArbitrage {
    base_arbitrage: BaseArbitrage,
    paths: Arc<Mutex<Vec<Vec<(Arc<Box<dyn Token>>, Arc<Box<dyn Dex>>, f64, f64)>>>>,
    num_swaps: usize,
}

struct PathInfo<'a> {
    path: &'a mut Vec<(Arc<Box<dyn Token>>, Arc<Box<dyn Dex>>, f64, f64)>,
    num_swaps: usize,
}

impl TriangleArbitrage {
    pub fn new(
        amount: f64,
        allowance_factor: f64,
        tokens: Arc<Vec<Box<dyn Token>>>,
        base_token: Arc<Box<dyn Token>>,
        dexes: Arc<Vec<Box<dyn Dex>>>,
        skip_write: bool,
        num_swaps: usize,
    ) -> Self {
        Self {
            base_arbitrage: BaseArbitrage::new(
                amount,
                allowance_factor,
                tokens,
                base_token,
                dexes,
                skip_write,
            ),
            paths: Arc::new(Mutex::new(Vec::new())),
            num_swaps,
        }
    }

    pub fn amount(&self) -> f64 {
        self.base_arbitrage.amount()
    }

    pub fn tokens(&self) -> Arc<Vec<Box<dyn Token>>> {
        self.base_arbitrage.tokens()
    }

    pub fn base_token(&self) -> Arc<Box<dyn Token>> {
        self.base_arbitrage.base_token()
    }

    pub fn dexes(&self) -> Arc<Vec<Box<dyn Dex>>> {
        self.base_arbitrage.dexes()
    }

    pub fn skip_write(&self) -> bool {
        self.base_arbitrage.skip_write()
    }

    #[async_recursion]
    async fn find_arbitrage_paths(
        tokens: &Arc<Vec<Box<dyn Token>>>,
        dexes: &Arc<Vec<Box<dyn Dex>>>,
        paths: &Arc<Mutex<Vec<Vec<(Arc<Box<dyn Token>>, Arc<Box<dyn Dex>>, f64, f64)>>>>,
        start_token: &Arc<Box<dyn Token>>,
        visited: &mut HashSet<(String, String)>,
        path: &mut Vec<(Arc<Box<dyn Token>>, Arc<Box<dyn Dex>>, f64, f64)>,
        num_swaps: usize,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(first_path) = path.first() {
            if path.len() >= 2
                && path.len() <= num_swaps
                && first_path.0.symbol_name() == (*start_token).symbol_name()
            {
                let mut paths_lock = paths.lock().await;
                paths_lock.push(path.clone());
                return Ok(());
            }
        }

        for token in tokens.iter() {
            let token_arc = Arc::new(token.clone());
            for dex in dexes.iter() {
                let dex_arc = Arc::new(dex.clone());

                // skip visited edges
                let token_dex_pair = (
                    String::from(token_arc.symbol_name()),
                    String::from(dex_arc.name()),
                );
                if visited.contains(&token_dex_pair) {
                    continue;
                }

                // check if the token pair is available for trading
                let token_price_result = dex_arc
                    .get_token_price(&TokenPair::new(start_token.clone(), token_arc.clone()), 1.0)
                    .await;
                if let Err(_) = &token_price_result {
                    continue;
                }

                // add the edge to the visited set and path
                visited.insert(token_dex_pair.clone());
                path.push((
                    token_arc.clone(),
                    dex_arc.clone(),
                    token_price_result.unwrap(),
                    0.0,
                ));

                // recursively explore the remaining path
                Self::find_arbitrage_paths(
                    tokens, dexes, paths, &token_arc, visited, path, num_swaps,
                )
                .await?;

                // remove the edge from the visited set and path
                visited.remove(&token_dex_pair);
                path.pop();
            }
        }
        Ok(())
    }

    async fn calculate_arbitrage_profit(
        path: &[(Arc<Box<dyn Token>>, Arc<Box<dyn Dex>>, f64, f64)],
        amount: f64,
    ) -> Option<f64> {
        let mut remaining_amount = amount;
        let path_len = path.len();
        let mut swap_amounts = Vec::with_capacity(path_len);

        for i in 0..path_len {
            let token_a = &path[i].0;
            let token_b = if i == path_len - 1 {
                &path[0].0
            } else {
                &path[i + 1].0
            };

            let dex = &path[i].1;
            let token_pair = TokenPair::new(Arc::clone(token_a), Arc::clone(token_b));
            match dex.get_token_price(&token_pair, remaining_amount).await {
                Ok(output_amount) => {
                    remaining_amount = output_amount;
                    swap_amounts.push(remaining_amount);
                }
                Err(_) => return None,
            }
        }

        let profit = remaining_amount - amount;

        if profit > 0.0 {
            Some(profit)
        } else {
            None
        }
    }
}

#[async_trait]
impl Arbitrage for TriangleArbitrage {
    async fn find_opportunities(
        &self,
    ) -> Result<Vec<ArbitrageOpportunity>, Box<dyn Error + Send + Sync>> {
        let amount = self.amount();
        let num_swaps = self.num_swaps;
        let dexes = Arc::clone(&self.dexes());
        let paths = Arc::clone(&self.paths);
        let tokens = Arc::clone(&self.tokens());

        // Find arbitrage paths for each token
        let find_path_tasks: Vec<_> = tokens
            .iter()
            .enumerate()
            .map(|(token_index, token)| {
                let dexes_clone = Arc::clone(&dexes);
                let paths_clone = Arc::clone(&paths);
                let tokens_clone = Arc::clone(&tokens);
                let token_clone = Arc::new(token.clone());

                tokio::spawn(async move {
                    let mut visited_pairs: HashSet<(String, String)> = HashSet::new();
                    for dex_1 in dexes_clone.iter() {
                        let start_token = Arc::clone(&token_clone);
                        let mut path: Vec<(Arc<Box<dyn Token>>, Arc<Box<dyn Dex>>, f64, f64)> =
                            Vec::new();
                        let _ = TriangleArbitrage::find_arbitrage_paths(
                            &tokens_clone,
                            &dexes_clone,
                            &paths_clone,
                            &start_token,
                            &mut visited_pairs,
                            &mut path,
                            num_swaps,
                        )
                        .await;
                    }
                })
            })
            .collect();

        // Wait for all path finding tasks to finish
        let _ = futures::future::join_all(find_path_tasks).await;

        // Get arbitrage paths for profit calculation
        let paths_for_profit_calculation = paths.lock().await.clone();

        // Calculate arbitrage profits for each path
        let profit_calculation_tasks: Vec<_> = paths_for_profit_calculation
            .iter()
            .cloned()
            .map(|path| {
                let amount_clone = amount;
                tokio::spawn(async move {
                    TriangleArbitrage::calculate_arbitrage_profit(&path, amount_clone).await
                })
            })
            .collect();

        // Wait for all profit calculation tasks to finish
        let profit_results = futures::future::join_all(profit_calculation_tasks).await;

        // Combine the paths and their corresponding profit results into ArbitrageOpportunities
        let mut results = Vec::new();
        for (path, profit_result) in paths_for_profit_calculation
            .iter()
            .zip(profit_results.into_iter())
        {
            if let Ok(Some(profit)) = profit_result {
                let dex1_index =
                    find_index(&**dexes, |dex| dex.name() == path[0].1.name()).unwrap();
                let dex2_index =
                    find_index(&**dexes, |dex| dex.name() == path[1].1.name()).unwrap();
                let dex3_index = if path.len() > 2 {
                    find_index(&**dexes, |dex| dex.name() == path[2].1.name())
                } else {
                    None
                };

                let token_a_index =
                    find_index(&**tokens, |t| t.symbol_name() == path[0].0.symbol_name()).unwrap();
                let token_b_index =
                    find_index(&**tokens, |t| t.symbol_name() == path[1].0.symbol_name()).unwrap();
                let token_c_index = if path.len() > 2 {
                    find_index(&**tokens, |t| t.symbol_name() == path[2].0.symbol_name())
                } else {
                    None
                };

                let has_opportunity = profit > 0.0;
                if has_opportunity {
                    let opportunity = ArbitrageOpportunity {
                        dex1_index,
                        dex2_index,
                        dex3_index,
                        token_a_index,
                        token_b_index,
                        token_c_index,
                        profit,
                        amount,
                    };

                    results.push(opportunity);
                }

                log_arbitrage_info_helper(
                    &**dexes,
                    &**tokens,
                    &path
                        .iter()
                        .map(|(token, dex, price_in, price_out)| {
                            (
                                find_index(&**tokens, |t| t.symbol_name() == token.symbol_name())
                                    .unwrap(),
                                find_index(&**dexes, |d| d.name() == dex.name()).unwrap(),
                                *price_in,
                                *price_out,
                            )
                        })
                        .collect::<Vec<(usize, usize, f64, f64)>>()
                        .as_slice(),
                    dex1_index,
                    dex2_index,
                    dex3_index,
                    token_a_index,
                    token_b_index,
                    token_c_index,
                    amount,
                    profit,
                    has_opportunity,
                );
            }
        }
        Ok(results)
    }

    async fn execute_transactions(
        &self,
        opportunity: &ArbitrageOpportunity,
        wallet_and_provider: &Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
        transaction_results: Arc<RwLock<Vec<TransactionResult>>>,
        log_limit: usize,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    async fn init(&self, owner: Address) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_arbitrage.init(owner).await
    }
}

fn log_arbitrage_info_helper(
    dexes_clone: &[Box<dyn Dex>],
    tokens_clone: &[Box<dyn Token>],
    path: &[(usize, usize, f64, f64)],
    dex1_index: usize,
    dex2_index: usize,
    dex3_index: Option<usize>,
    token_a_index: usize,
    token_b_index: usize,
    token_c_index: Option<usize>,
    amount: f64,
    profit: f64,
    has_opportunity: bool,
) {
    let dex_1 = &dexes_clone[dex1_index];
    let dex_2 = &dexes_clone[dex2_index];
    let dex_3 = dex3_index.map(|index| &dexes_clone[index]);

    let token_a = &tokens_clone[token_a_index];
    let token_b = &tokens_clone[token_b_index];
    let token_c = token_c_index.map(|index| &tokens_clone[index]);

    TriangleArbitrage::log_arbitrage_info(
        dex_1,
        dex_2,
        dex_3,
        token_a,
        token_b,
        token_c,
        amount,
        path[0].3,
        path[1].3,
        if let Some(path_item) = path.get(2) {
            Some(path_item.3)
        } else {
            None
        },
        path[0].2,
        path[1].2,
        if let Some(path_item) = path.get(2) {
            Some(path_item.2)
        } else {
            None
        },
        profit,
        has_opportunity,
    );
}
