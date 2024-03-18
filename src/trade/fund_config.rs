use debot_market_analyzer::TradingStrategy;
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST: &[&str] = &[
    "BTC-USD", "ETH-USD", "SOL-USD", "SUI-USD", "APT-USD", "ARB-USD",
];

lazy_static! {
    static ref FUND_SCALE_FACTOR: Decimal = env::var("FUND_SCALE_FACTOR")
        .ok()
        .and_then(|val| val.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(1, 0));
}

pub fn get(
    dex_name: &str,
    strategy: Option<&TradingStrategy>,
) -> Vec<(String, TradingStrategy, Decimal, Decimal, Decimal, Decimal)> {
    match dex_name {
        "rabbitx" | "hyperliquid" => vec![
            (
                TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::Rebalance,
                Decimal::new(2000, 0), // initial amount (in USD)
                Decimal::new(5, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::MarketMake,
                Decimal::new(2000, 0), // initial amount (in USD)
                Decimal::new(25, 2),   // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
        ]
        .into_iter()
        .filter(|(_, token_strategy, _, _, _, _)|
                // Check if strategy is None or if it matches the token's strategy
                strategy.is_none() || strategy == Some(token_strategy))
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
        .collect(),
        _ => panic!("Unsupported dex"),
    }
}
