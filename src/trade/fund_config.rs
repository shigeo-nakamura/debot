use debot_market_analyzer::{TradingStrategy, TrendType};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST: &[&str] = &[
    "BTC-USD", "ETH-USD", "SOL-USD", "ARB-USD", "BNB-USD", "SUI-USD", "APT-USD", "OP-USD",
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
    let is_trim_position = false;
    match dex_name {
        "rabbitx" | "hyperliquid" => vec![
            (
                TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(3000, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(3000, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[1].to_owned(), // ETH
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(3000, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[1].to_owned(), // ETH
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(3000, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[2].to_owned(), // SOL
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(3000, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[2].to_owned(), // SOL
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(3000, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(2, 0),    // risk reward
                Decimal::new(1, 3),    // loss cut ratio
            ),
            (
                TOKEN_LIST[3].to_owned(), // ARB
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[3].to_owned(), // ARB
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[4].to_owned(), // BNB
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[4].to_owned(), // BNB
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[5].to_owned(), // SUI
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[5].to_owned(), // SUI
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[6].to_owned(), // APT
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[6].to_owned(), // APT
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[7].to_owned(), // OP
                TradingStrategy::TrendFollow(TrendType::Up, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[7].to_owned(), // OP
                TradingStrategy::TrendFollow(TrendType::Down, is_trim_position),
                Decimal::new(600, 0), // initial amount (in USD)
                Decimal::new(8, 1),   // position size ratio
                Decimal::new(2, 0),   // risk reward
                Decimal::new(1, 3),   // loss cut ratio
            ),
            (
                TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::MarketMake,
                Decimal::new(2000, 0), // initial amount (in USD)
                Decimal::new(25, 2),   // position size ratio
                Decimal::new(1, 0),    // risk reward
                Decimal::new(5, 4),    // loss cut ratio
            ),
        ]
        .into_iter()
        .filter(|(_, trading_strategy, _, _, _, _)|
                // Check if strategy is None or if it matches the token's strategy
                strategy.is_none() || strategy == Some(trading_strategy))
        .map(
            |(token, trading_strategy, amount, size_ratio, risk_reward, loss_cut_ratio)| {
                (
                    token,
                    trading_strategy,
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
