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
    Decimal,
    Decimal,
    Option<Decimal>,
)> {
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
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                        None,                                 // pair token
                        TradingStrategy::MeanReversion(TrendType::Up),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(2, 1),     // position size ratio
                        Decimal::new(1, 3),     // take profit ratio
                        Decimal::new(5, 3),     // loss cut ratio
                        Decimal::new(2, 1),     // RSI lower threshold
                        Decimal::new(8, 1),     // RSI higher threshold
                        Some(atr_ratio),        // ATR ratio
                    ),
                    (
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(), //  BTC
                        None,                                 // pair token
                        TradingStrategy::MeanReversion(TrendType::Down),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(2, 1),     // position size ratio
                        Decimal::new(1, 3),     // take profit ratio
                        Decimal::new(5, 3),     // loss cut ratio
                        Decimal::new(2, 1),     // RSI lower threshold
                        Decimal::new(8, 1),     // RSI higher threshold
                        Some(atr_ratio),        // ATR ratio
                    ),
                    (
                        HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                        None,                                 // pair token
                        TradingStrategy::MeanReversion(TrendType::Up),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(2, 1),     // position size ratio
                        Decimal::new(1, 3),     // take profit ratio
                        Decimal::new(5, 3),     // loss cut ratio
                        Decimal::new(2, 1),     // RSI lower threshold
                        Decimal::new(8, 1),     // RSI higher threshold
                        Some(atr_ratio),        // ATR ratio
                    ),
                    (
                        HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                        None,                                 // pair token
                        TradingStrategy::MeanReversion(TrendType::Down),
                        Decimal::new(10000, 0), // initial amount (in USD)
                        Decimal::new(2, 1),     // position size ratio
                        Decimal::new(1, 3),     // take profit ratio
                        Decimal::new(5, 3),     // loss cut ratio
                        Decimal::new(2, 1),     // RSI lower threshold
                        Decimal::new(8, 1),     // RSI higher threshold
                        Some(atr_ratio),        // ATR ratio
                    ),
                ]
            })
            .collect::<Vec<_>>(),
        _ => panic!("Unsupported dex"),
    };

    // Add non-repeating items
    let non_repeating_items = vec![];

    strategy_list.extend(non_repeating_items);

    strategy_list
        .into_iter()
        .filter(|(_, _, trading_strategy, _, _, _, _, _, _, _)| {
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
                rsi_lower_threshold,
                rsi_upper_threshold,
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
                    rsi_lower_threshold,
                    rsi_upper_threshold,
                    atr_ratio,
                )
            },
        )
        .collect()
}
