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

    let atr_values = vec![
        Decimal::new(5, 2),
        Decimal::new(10, 2),
        Decimal::new(15, 2),
        Decimal::new(20, 2),
        Decimal::new(25, 2),
        Decimal::new(30, 2),
        Decimal::new(35, 2),
        Decimal::new(40, 2),
        Decimal::new(45, 2),
        Decimal::new(50, 2),
    ];

    let mut strategy_list = match dex_name {
        "hyperliquid" => atr_values
            .into_iter()
            .flat_map(|atr_ratio| {
                vec![
                    (
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                        Some(HYPERLIQUID_TOKEN_LIST[1].to_owned()), // pair token ETH
                        TradingStrategy::TrendFollow(TrendType::Up),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(4, 1),     // position size ratio
                        Decimal::new(1, 3),     // least take profit ratio
                        Decimal::new(3, 2),     // loss cut ratio
                        Some(atr_ratio),        // ATR ratio
                    ),
                    (
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                        Some(HYPERLIQUID_TOKEN_LIST[2].to_owned()), // pair token SOL
                        TradingStrategy::TrendFollow(TrendType::Up),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(4, 1),     // position size ratio
                        Decimal::new(1, 3),     // least take profit ratio
                        Decimal::new(3, 2),     // loss cut ratio
                        Some(atr_ratio),        // ATR ratio
                    ),
                    (
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                        Some(HYPERLIQUID_TOKEN_LIST[3].to_owned()), // pair token BNB
                        TradingStrategy::TrendFollow(TrendType::Down),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(4, 1),     // position size ratio
                        Decimal::new(1, 3),     // least take profit ratio
                        Decimal::new(3, 2),     // loss cut ratio
                        Some(atr_ratio),        // ATR ratio
                    ),
                    (
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(),       // BTC
                        Some(HYPERLIQUID_TOKEN_LIST[4].to_owned()), // pair token SUI
                        TradingStrategy::TrendFollow(TrendType::Down),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(4, 1),     // position size ratio
                        Decimal::new(1, 3),     // least take profit ratio
                        Decimal::new(3, 2),     // loss cut ratio
                        Some(atr_ratio),        // ATR ratio
                    ),
                ]
            })
            .collect::<Vec<_>>(),
        _ => panic!("Unsupported dex"),
    };

    // Add non-repeating items
    let non_repeating_items = vec![
        (
            HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
            None,                                 // pair token
            TradingStrategy::PassiveTrade(full_hedge),
            Decimal::new(10000, 0), // initial amount (in USD)
            Decimal::new(4, 1),     // position size ratio
            Decimal::new(6, 2),     // least take profit ratio
            Decimal::new(3, 2),     // loss cut ratio
            None,                   // ATR ratio
        ),
        (
            HYPERLIQUID_TOKEN_LIST[2].to_owned(), // SOL
            None,                                 // pair token
            TradingStrategy::PassiveTrade(full_hedge),
            Decimal::new(10000, 0), // initial amount (in USD)
            Decimal::new(4, 1),     // position size ratio
            Decimal::new(6, 2),     // least take profit ratio
            Decimal::new(3, 2),     // loss cut ratio
            None,                   // ATR ratio
        ),
        (
            HYPERLIQUID_TOKEN_LIST[3].to_owned(), // BNB
            None,                                 // pair token
            TradingStrategy::PassiveTrade(full_hedge),
            Decimal::new(10000, 0), // initial amount (in USD)
            Decimal::new(4, 1),     // position size ratio
            Decimal::new(6, 2),     // least take profit ratio
            Decimal::new(3, 2),     // loss cut ratio
            None,                   // ATR ratio
        ),
        (
            HYPERLIQUID_TOKEN_LIST[4].to_owned(), // SUI
            None,                                 // pair token
            TradingStrategy::PassiveTrade(full_hedge),
            Decimal::new(10000, 0), // initial amount (in USD)
            Decimal::new(4, 1),     // position size ratio
            Decimal::new(6, 2),     // least take profit ratio
            Decimal::new(3, 2),     // loss cut ratio
            None,                   // ATR ratio
        ),
    ];

    strategy_list.extend(non_repeating_items);

    strategy_list
        .into_iter()
        .filter(|(_, _, trading_strategy, _, _, _, _, _)| {
            strategy.is_none() || strategy == Some(trading_strategy)
        })
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
