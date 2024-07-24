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
    TradingStrategy,
    Decimal,
    Decimal,
    Option<Decimal>,
    Option<Decimal>,
    i64,
)> {
    let take_profit_ratio_values = vec![
        None,
        Some(Decimal::new(5, 3)),
        Some(Decimal::new(1, 2)),
        Some(Decimal::new(2, 2)),
    ];

    let atr_spread_values = vec![
        None,
        Some(Decimal::new(125, 3)),
        Some(Decimal::new(25, 2)),
        Some(Decimal::new(5, 1)),
        Some(Decimal::ONE),
    ];

    let open_hours_values = vec![3, 6, 12, 24];

    let mut strategy_list = Vec::new();

    if dex_name == "hyperliquid" {
        for take_profit_ratio in take_profit_ratio_values {
            for atr_spread in atr_spread_values.clone() {
                if take_profit_ratio.is_none() && atr_spread.is_none() {
                    continue;
                }
                for open_hours in &open_hours_values {
                    strategy_list.push((
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                        TradingStrategy::RandomWalk(TrendType::Up),
                        Decimal::new(5000, 0), // initial amount (in USD)
                        Decimal::new(8, 1),    // position size ratio
                        take_profit_ratio,     // take profit ratioat
                        atr_spread,            // spread by ATR
                        open_hours,            // max open hours
                    ));

                    strategy_list.push((
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                        TradingStrategy::RandomWalk(TrendType::Down),
                        Decimal::new(5000, 0), // initial amount (in USD)
                        Decimal::new(8, 1),    // position size ratio
                        take_profit_ratio,     // take profit ratioat
                        atr_spread,            // spread by ATR
                        open_hours,            // max open hours
                    ));

                    strategy_list.push((
                        HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                        TradingStrategy::RandomWalk(TrendType::Up),
                        Decimal::new(5000, 0), // initial amount (in USD)
                        Decimal::new(8, 1),    // position size ratio
                        take_profit_ratio,     // take profit ratio
                        atr_spread,            // spread by ATR
                        open_hours,            // max open hours
                    ));

                    strategy_list.push((
                        HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                        TradingStrategy::RandomWalk(TrendType::Down),
                        Decimal::new(5000, 0), // initial amount (in USD)
                        Decimal::new(8, 1),    // position size ratio
                        take_profit_ratio,     // take profit ratio
                        atr_spread,            // spread by ATR
                        open_hours,            // max open hours
                    ));

                    strategy_list.push((
                        HYPERLIQUID_TOKEN_LIST[0].to_owned(), // BTC
                        TradingStrategy::MachineLearning(TrendType::Up),
                        Decimal::new(5000, 0), // initial amount (in USD)
                        Decimal::new(8, 1),    // position size ratio
                        take_profit_ratio,     // take profit ratioat
                        atr_spread,            // spread by ATR
                        open_hours,            // max open hours
                    ));

                    strategy_list.push((
                        HYPERLIQUID_TOKEN_LIST[1].to_owned(), // ETH
                        TradingStrategy::MachineLearning(TrendType::Down),
                        Decimal::new(5000, 0), // initial amount (in USD)
                        Decimal::new(8, 1),    // position size ratio
                        take_profit_ratio,     // take profit ratio
                        atr_spread,            // spread by ATR
                        open_hours,            // max open hours
                    ));
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
        .filter(|(_, trading_strategy, _, _, _, _, _)| {
            strategy.is_none() || strategy == Some(trading_strategy)
        })
        .map(
            |(
                token,
                trading_strategy,
                amount,
                size_ratio,
                take_profit_ratio,
                atr_spread,
                open_hours,
            )| {
                (
                    token,
                    trading_strategy,
                    amount * *FUND_SCALE_FACTOR,
                    size_ratio,
                    take_profit_ratio,
                    atr_spread,
                    *open_hours,
                )
            },
        )
        .collect()
}
