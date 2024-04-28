use debot_market_analyzer::{TradingStrategy, TrendType};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST_SIZE: u32 = 12;

pub const HYPERLIQUID_TOKEN_LIST: &[&str] = &[
    "BTC-USD",
    "ETH-USD",
    "BNB-USD",
    "SOL-USD",
    "AVAX-USD",
    "BCH-USD",
    "SUI-USD",
    "APT-USD",
    "ARB-USD",
    "OP-USD",
    "MATIC-USD",
    "NEAR-USD",
];

pub const RABBITX_TOKEN_LIST: &[&str] = &["BTC-USD", "ETH-USD"];

lazy_static! {
    static ref FUND_SCALE_FACTOR: Decimal = env::var("FUND_SCALE_FACTOR")
        .ok()
        .and_then(|val| val.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(1, 0));
}

pub fn get(
    dex_name: &str,
    strategy: Option<&TradingStrategy>,
) -> Vec<(
    String,
    Option<String>,
    TradingStrategy,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
)> {
    let no_hedge = false;
    let hedge_position = true;
    let loss_cut_ratio = Decimal::new(3, 2);
    let strategy_list = match dex_name {
        "rabbitx" => vec![
            (
                RABBITX_TOKEN_LIST[0].to_owned(),       // BTC
                Some(RABBITX_TOKEN_LIST[1].to_owned()), // pair token
                TradingStrategy::TrendFollow(TrendType::Up, no_hedge),
                Decimal::new(1500, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(1, 3),    // take profit ratio
                loss_cut_ratio,        // loss cut ratio
            ),
            (
                RABBITX_TOKEN_LIST[0].to_owned(),       // BTC
                Some(RABBITX_TOKEN_LIST[1].to_owned()), // pair token
                TradingStrategy::TrendFollow(TrendType::Down, no_hedge),
                Decimal::new(1500, 0), // initial amount (in USD)
                Decimal::new(8, 1),    // position size ratio
                Decimal::new(1, 3),    // take profit ratio
                loss_cut_ratio,        // loss cut ratio
            ),
            (
                RABBITX_TOKEN_LIST[0].to_owned(), // BTC
                None,
                TradingStrategy::MarketMake,
                Decimal::new(2000, 0), // initial amount (in USD)
                Decimal::new(25, 2),   // position size ratio
                Decimal::new(1, 0),    // risk reward
                Decimal::new(5, 4),    // loss cut ratio
            ),
        ],
        "hyperliquid" => vec![
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token
                TradingStrategy::TrendFollow(TrendType::Up, no_hedge),
                Decimal::new(5000, 0), // initial amount (in USD)
                Decimal::new(4, 1),    // position size ratio
                Decimal::new(1, 3),    // take profit ratio
                loss_cut_ratio,        // loss cut ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token
                TradingStrategy::TrendFollow(TrendType::Up, no_hedge),
                Decimal::new(5000, 0), // initial amount (in USD)
                Decimal::new(4, 1),    // position size ratio
                Decimal::new(1, 3),    // take profit ratio
                loss_cut_ratio,        // loss cut ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token
                TradingStrategy::TrendFollow(TrendType::Down, no_hedge),
                Decimal::new(5000, 0), // initial amount (in USD)
                Decimal::new(4, 1),    // position size ratio
                Decimal::new(1, 3),    // take profit ratio
                loss_cut_ratio,        // loss cut ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token
                TradingStrategy::TrendFollow(TrendType::Down, no_hedge),
                Decimal::new(5000, 0), // initial amount (in USD)
                Decimal::new(4, 1),    // position size ratio
                Decimal::new(1, 3),    // take profit ratio
                loss_cut_ratio,        // loss cut ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                None,                                 // pair token
                TradingStrategy::PassiveTrade,
                Decimal::new(10000, 0), // initial amount (in USD)
                Decimal::new(4, 1),     // position size ratio
                Decimal::new(1, 3),     // take profit ratio
                loss_cut_ratio,         // loss cut ratio
            ),
        ],
        _ => panic!("Unsupported dex"),
    };

    strategy_list
        .into_iter()
        .filter(|(_, _, trading_strategy, _, _, _, _)|
                // Check if strategy is None or if it matches the token's strategy
                strategy.is_none() || strategy == Some(trading_strategy))
        .map(
            |(
                token,
                pair_token,
                trading_strategy,
                amount,
                size_ratio,
                take_profit_ratio,
                loss_cut_ratio,
            )| {
                (
                    token,
                    pair_token,
                    trading_strategy,
                    amount * *FUND_SCALE_FACTOR,
                    size_ratio,
                    take_profit_ratio,
                    loss_cut_ratio,
                )
            },
        )
        .collect()
}
