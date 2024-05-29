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
    Option<Decimal>,
)> {
    let take_profit_ratio_values = vec![
        Decimal::new(1, 3),
        Decimal::new(2, 3),
        Decimal::new(3, 3),
        Decimal::new(4, 3),
        Decimal::new(5, 3),
        Decimal::new(6, 3),
        Decimal::new(7, 3),
        Decimal::new(8, 3),
        Decimal::new(9, 3),
    ];

    let atr_spread_values = vec![
        None,
        Some(Decimal::new(1, 1)),
        Some(Decimal::new(2, 1)),
        Some(Decimal::new(3, 1)),
        Some(Decimal::new(4, 1)),
        Some(Decimal::new(5, 1)),
    ];

    let mut strategy_list = Vec::new();

    if dex_name == "hyperliquid" {
        for take_profit_ratio in take_profit_ratio_values {
            for atr_spread in atr_spread_values.clone() {
                strategy_list.push((
                    HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                    None,                                 // pair token
                    TradingStrategy::RandomWalk(TrendType::Up),
                    Decimal::new(5000, 0), // initial amount (in USD)
                    Decimal::new(8, 1),    // position size ratio
                    take_profit_ratio,     // take profit ratioat
                    atr_spread,            // spread by ATR
                ));

                strategy_list.push((
                    HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                    None,                                 // pair token
                    TradingStrategy::RandomWalk(TrendType::Down),
                    Decimal::new(5000, 0), // initial amount (in USD)
                    Decimal::new(8, 1),    // position size ratio
                    take_profit_ratio,     // take profit ratio
                    atr_spread,            // spread by ATR
                ));

                strategy_list.push((
                    HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                    None,                                 // pair token
                    TradingStrategy::RandomWalk(TrendType::Up),
                    Decimal::new(5000, 0), // initial amount (in USD)
                    Decimal::new(8, 1),    // position size ratio
                    take_profit_ratio,     // take profit ratio
                    atr_spread,            // spread by ATR
                ));

                strategy_list.push((
                    HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                    None,                                 // pair token
                    TradingStrategy::RandomWalk(TrendType::Down),
                    Decimal::new(5000, 0), // initial amount (in USD)
                    Decimal::new(8, 1),    // position size ratio
                    take_profit_ratio,     // take profit ratio
                    atr_spread,            // spread by ATR
                ));
            }
        }
    } else {
        panic!("Unsupported dex");
    }

    // Add non-repeating items if any
    let non_repeating_items = vec![];

    strategy_list.extend(non_repeating_items);

    strategy_list
        .into_iter()
        .filter(|(_, _, trading_strategy, _, _, _, _)| {
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
                atr_spread,
            )| {
                (
                    token,
                    pair_token,
                    trading_strategy,
                    amount * *FUND_SCALE_FACTOR,
                    size_ratio,
                    take_profit_ratio,
                    atr_spread,
                )
            },
        )
        .collect()
}
