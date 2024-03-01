use std::env;

use debot_market_analyzer::TradingStrategy;
use lazy_static::lazy_static;

pub const TOKEN_LIST_SIZE: usize = 6;

pub const RABBITX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = [
    "BTC-USD", "ETH-USD", "SOL-USD", "SUI-USD", "APT-USD", "ARB-USD",
];

lazy_static! {
    static ref FUND_SCALE_FACTOR: f64 = {
        match env::var("FUND_SCALE_FACTOR") {
            Ok(val) => val.parse::<f64>().unwrap_or(1.0),
            Err(_) => 1.0,
        }
    };
}

pub fn get(
    dex_name: &str,
    strategy: Option<&TradingStrategy>,
) -> Vec<(String, TradingStrategy, f64, f64, f64, f64)> {
    if dex_name == "rabbitx" {
        let all_funds = vec![
            (
                RABBITX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::Rebalance,
                100.0, // initial amount(in USD)
                0.5,   // position size ration
                0.0,   // risk reward
                0.0,   // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[1].to_owned(), // ETH
                TradingStrategy::Rebalance,
                100.0, // initial amount(in USD)
                0.5,   // position size ration
                0.0,   // risk reward
                0.0,   // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[2].to_owned(), // SOL
                TradingStrategy::Rebalance,
                100.0, // initial amount(in USD)
                0.5,   // position size ration
                0.0,   // risk reward
                0.0,   // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[3].to_owned(), // SUI
                TradingStrategy::Rebalance,
                100.0, // initial amount(in USD)
                0.5,   // position size ration
                0.0,   // risk reward
                0.0,   // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[4].to_owned(), // APT
                TradingStrategy::Rebalance,
                100.0, // initial amount(in USD)
                0.5,   // position size ration
                0.0,   // risk reward
                0.0,   // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[5].to_owned(), // ARB
                TradingStrategy::Rebalance,
                100.0, // initial amount(in USD)
                0.5,   // position size ration
                0.0,   // risk reward
                0.0,   // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow,
                100.0, // initial amount(in USD)
                0.1,   // position size ration
                2.0,   // risk reward
                0.01,  // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[1].to_owned(), // ETH
                TradingStrategy::TrendFollow,
                100.0, // initial amount(in USD)
                0.1,   // position size ration
                2.0,   // risk reward
                0.01,  // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[2].to_owned(), // SOL
                TradingStrategy::TrendFollow,
                100.0, // initial amount(in USD)
                0.1,   // position size ration
                2.0,   // risk reward
                0.01,  // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[3].to_owned(), // SUI
                TradingStrategy::TrendFollow,
                100.0, // initial amount(in USD)
                0.1,   // position size ration
                2.0,   // risk reward
                0.01,  // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[4].to_owned(), // APT
                TradingStrategy::TrendFollow,
                100.0, // initial amount(in USD)
                0.1,   // position size ration
                2.0,   // risk reward
                0.01,  // loss cut ration
            ),
            (
                RABBITX_TOKEN_LIST[5].to_owned(), // ARB
                TradingStrategy::TrendFollow,
                100.0, // initial amount(in USD)
                0.1,   // position size ration
                2.0,   // risk reward
                0.01,  // loss cut ration
            ),
        ];

        all_funds
            .into_iter()
            .filter(|(_, token_strategy, _, _, _, _)|
                // Check if strategy is None or if it matches the token's strategy
                strategy.is_none() || strategy == Some(&token_strategy))
            .map(
                |(token, token_strategy, amount, size_ratio, risk_reward, loss_cut_ratio)| {
                    (
                        token,
                        token_strategy,
                        amount * *FUND_SCALE_FACTOR,
                        size_ratio,
                        risk_reward,
                        loss_cut_ratio,
                    )
                },
            )
            .collect()
    } else {
        panic!("Unsupported dex");
    }
}
