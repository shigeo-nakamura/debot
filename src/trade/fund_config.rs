use debot_market_analyzer::{TradingStrategy, TrendType};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST_SIZE: u32 = 12;

pub const HYPERLIQUID_TOKEN_LIST: &[&str] = &[
    "BTC-USD",
    "ETH-USD",
    "SOL-USD",
    "BNB-USD",
    "SUI-USD",
    "AVAX-USD",
    "BCH-USD",
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
    Option<Decimal>,
)> {
    let full_hedge = Decimal::new(1, 0);

    let strategy_list = match dex_name {
        "hyperliquid" => vec![
            // ATR 0.5
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token ETH
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(5, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[2].to_owned()), // pair token SOL
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(5, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[3].to_owned()), // pair token BNB
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(5, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[4].to_owned()), // pair token SUI
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(5, 1)), // ATR ratio
            ),
            // ATR 0.4
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token ETH
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(4, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[2].to_owned()), // pair token SOL
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(4, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[3].to_owned()), // pair token BNB
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(4, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[4].to_owned()), // pair token SUI
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(4, 1)), // ATR ratio
            ),
            // ATR 0.3
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token ETH
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(3, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[2].to_owned()), // pair token SOL
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(3, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[3].to_owned()), // pair token BNB
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(3, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[4].to_owned()), // pair token SUI
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(3, 1)), // ATR ratio
            ),
            // ATR 0.2
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token ETH
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(2, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[2].to_owned()), // pair token SOL
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(2, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[3].to_owned()), // pair token BNB
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(2, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[4].to_owned()), // pair token SUI
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(2, 1)), // ATR ratio
            ),
            // ATR 0.1
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token ETH
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(1, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[2].to_owned()), // pair token SOL
                TradingStrategy::TrendFollow(TrendType::Up),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(1, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[3].to_owned()), // pair token BNB
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(1, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                Some(HYPERLIQUID_TOKEN_LIST[4].to_owned()), // pair token SUI
                TradingStrategy::TrendFollow(TrendType::Down),
                Decimal::new(10000, 0),   // initial amount (in USD)
                Decimal::new(4, 1),       // position size ratio
                Decimal::new(1, 3),       // take profit ratio
                Decimal::new(3, 2),       // loss cut ratio
                Some(Decimal::new(1, 1)), // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[1].to_owned(),       // ETH
                Some(HYPERLIQUID_TOKEN_LIST[0].to_owned()), // pair token BTC
                TradingStrategy::PassiveTrade(full_hedge),
                Decimal::new(10000, 0), // initial amount (in USD)
                Decimal::new(4, 1),     // position size ratio
                Decimal::new(6, 2),     // take profit ratio
                Decimal::new(3, 2),     // loss cut ratio
                None,                   // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[2].to_owned(),       // SOL
                Some(HYPERLIQUID_TOKEN_LIST[0].to_owned()), // pair token BTC
                TradingStrategy::PassiveTrade(full_hedge),
                Decimal::new(10000, 0), // initial amount (in USD)
                Decimal::new(4, 1),     // position size ratio
                Decimal::new(6, 2),     // take profit ratio
                Decimal::new(3, 2),     // loss cut ratio
                None,                   // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[3].to_owned(),       // BNB
                Some(HYPERLIQUID_TOKEN_LIST[0].to_owned()), // pair token BTC
                TradingStrategy::PassiveTrade(full_hedge),
                Decimal::new(10000, 0), // initial amount (in USD)
                Decimal::new(4, 1),     // position size ratio
                Decimal::new(6, 2),     // take profit ratio
                Decimal::new(3, 2),     // loss cut ratio
                None,                   // ATR ratio
            ),
            (
                HYPERLIQUID_TOKEN_LIST[4].to_owned(),       // SUI
                Some(HYPERLIQUID_TOKEN_LIST[0].to_owned()), // pair token BTC
                TradingStrategy::PassiveTrade(full_hedge),
                Decimal::new(10000, 0), // initial amount (in USD)
                Decimal::new(4, 1),     // position size ratio
                Decimal::new(6, 2),     // take profit ratio
                Decimal::new(3, 2),     // loss cut ratio
                None,                   // ATR ratio
            ),
        ],
        _ => panic!("Unsupported dex"),
    };

    strategy_list
        .into_iter()
        .filter(|(_, _, trading_strategy, _, _, _, _,_)|
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
                atr_ratio,
            )| {
                (
                    token,
                    pair_token,
                    trading_strategy,
                    amount * *FUND_SCALE_FACTOR,
                    size_ratio,
                    take_profit_ratio,
                    loss_cut_ratio,
                    atr_ratio,
                )
            },
        )
        .collect()
}
