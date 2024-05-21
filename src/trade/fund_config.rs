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
    Decimal,
    f64,
    Option<Decimal>,
)> {
    let atr_values = vec![
        Decimal::new(125, 3),
        Decimal::new(250, 3),
        Decimal::new(375, 3),
        Decimal::new(500, 3),
    ];
    let adx_values = vec![
        Decimal::new(30, 2),
        Decimal::new(35, 2),
        Decimal::new(40, 2),
    ];
    let derivation_values = vec![1.0, 1.5, 2.0];
    let rsi_thresholds = vec![
        (Decimal::new(20, 2), Decimal::new(80, 2)),
        (Decimal::new(25, 2), Decimal::new(75, 2)),
        (Decimal::new(30, 2), Decimal::new(70, 2)),
    ];

    let mut strategy_list = Vec::new();

    if dex_name == "hyperliquid" {
        for atr_ratio in atr_values {
            for adx_threshold in &adx_values {
                for derivation in &derivation_values {
                    for rsi_threshold in &rsi_thresholds {
                        strategy_list.push((
                            HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                            None,                                 // pair token
                            TradingStrategy::MeanReversion(TrendType::Up),
                            Decimal::new(5000, 0), // initial amount (in USD)
                            Decimal::new(8, 1),    // position size ratio
                            Decimal::new(1, 3),    // least take profit ratio
                            Decimal::new(1, 2),    // loss cut ratio
                            rsi_threshold.0,       // RSI lower threshold
                            rsi_threshold.1,       // RSI higher threshold
                            *adx_threshold,        // ADX threshold
                            *derivation,           // derivation
                            Some(atr_ratio),       // ATR ratio
                        ));

                        strategy_list.push((
                            HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                            None,                                 // pair token
                            TradingStrategy::MeanReversion(TrendType::Down),
                            Decimal::new(5000, 0), // initial amount (in USD)
                            Decimal::new(8, 1),    // position size ratio
                            Decimal::new(1, 3),    // least take profit ratio
                            Decimal::new(1, 2),    // loss cut ratio
                            rsi_threshold.0,       // RSI lower threshold
                            rsi_threshold.1,       // RSI higher threshold
                            *adx_threshold,        // ADX threshold
                            *derivation,           // derivation
                            Some(atr_ratio),       // ATR ratio
                        ));

                        strategy_list.push((
                            HYPERLIQUID_TOKEN_LIST[1].to_owned(), //  ETH
                            None,                                 // pair token
                            TradingStrategy::MeanReversion(TrendType::Up),
                            Decimal::new(5000, 0), // initial amount (in USD)
                            Decimal::new(8, 1),    // position size ratio
                            Decimal::new(1, 3),    // least take profit ratio
                            Decimal::new(1, 2),    // loss cut ratio
                            rsi_threshold.0,       // RSI lower threshold
                            rsi_threshold.1,       // RSI higher threshold
                            *adx_threshold,        // ADX threshold
                            *derivation,           // derivation
                            Some(atr_ratio),       // ATR ratio
                        ));

                        strategy_list.push((
                            HYPERLIQUID_TOKEN_LIST[1].to_owned(), //  ETH
                            None,                                 // pair token
                            TradingStrategy::MeanReversion(TrendType::Down),
                            Decimal::new(5000, 0), // initial amount (in USD)
                            Decimal::new(8, 1),    // position size ratio
                            Decimal::new(1, 3),    // least take profit ratio
                            Decimal::new(1, 2),    // loss cut ratio
                            rsi_threshold.0,       // RSI lower threshold
                            rsi_threshold.1,       // RSI higher threshold
                            *adx_threshold,        // ADX threshold
                            *derivation,           // derivation
                            Some(atr_ratio),       // ATR ratio
                        ));
                    }
                }
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
        .filter(|(_, _, trading_strategy, _, _, _, _, _, _, _, _, _)| {
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
                adx_threshold,
                deviation,
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
                    adx_threshold,
                    deviation,
                    atr_ratio,
                )
            },
        )
        .collect()
}
