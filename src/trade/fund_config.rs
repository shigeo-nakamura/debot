use debot_market_analyzer::{TradingStrategy, TrendType};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST_SIZE: u32 = 3;

pub const TOKEN_LIST: &[&str] = &["BTC-USD", "ETH-USD", "SOL-USD"];

lazy_static! {
    static ref INITIAL_FUND_AMOUNT: Decimal = env::var("INITIAL_FUND_AMOUNT")
        .ok()
        .and_then(|val| val.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(50, 0));
}

pub fn get(
    dex_name: &str,
    strategy: Option<&TradingStrategy>,
) -> Vec<(
    String,
    TradingStrategy,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
    Option<Decimal>,
    i64,
)> {
    let take_profit_ratio_values = vec![
        None,
        Some(Decimal::new(500, 5)),
        Some(Decimal::new(100, 4)),
        Some(Decimal::new(200, 4)),
    ];

    let atr_spread_values = vec![
        None,
        Some(Decimal::new(100, 3)),
        Some(Decimal::new(200, 3)),
        Some(Decimal::new(300, 3)),
        Some(Decimal::new(400, 3)),
        Some(Decimal::new(500, 3)),
    ];

    let risk_reward_values = vec![Decimal::new(50, 2), Decimal::ONE, Decimal::new(200, 2)];

    let open_hours_values = vec![3, 6, 12, 24];

    let mut strategy_list = Vec::new();

    let initial_amount = *INITIAL_FUND_AMOUNT;

    if dex_name == "hyperliquid" {
        for take_profit_ratio in take_profit_ratio_values {
            for atr_spread in atr_spread_values.clone() {
                if take_profit_ratio.is_none() && atr_spread.is_none() {
                    continue;
                }
                for risk_reward in risk_reward_values.clone() {
                    for open_hours in &open_hours_values {
                        strategy_list.push((
                            TOKEN_LIST[0].to_owned(), // BTC
                            TradingStrategy::RandomWalk(TrendType::Up),
                            initial_amount,     // initial amount (in USD)
                            Decimal::new(8, 1), // position size ratio
                            risk_reward,
                            take_profit_ratio,
                            atr_spread, // spread by ATR
                            open_hours, // max open hours
                        ));

                        strategy_list.push((
                            TOKEN_LIST[0].to_owned(), // BTC
                            TradingStrategy::RandomWalk(TrendType::Down),
                            initial_amount,     // initial amount (in USD)
                            Decimal::new(8, 1), // position size ratio
                            risk_reward,
                            take_profit_ratio,
                            atr_spread, // spread by ATR
                            open_hours, // max open hours
                        ));

                        strategy_list.push((
                            TOKEN_LIST[0].to_owned(), // BTC
                            TradingStrategy::MeanReversion(TrendType::Up),
                            initial_amount,     // initial amount (in USD)
                            Decimal::new(8, 1), // position size ratio
                            risk_reward,
                            take_profit_ratio,
                            atr_spread, // spread by ATR
                            open_hours, // max open hours
                        ));

                        strategy_list.push((
                            TOKEN_LIST[0].to_owned(), // BTC
                            TradingStrategy::MeanReversion(TrendType::Down),
                            initial_amount,     // initial amount (in USD)
                            Decimal::new(8, 1), // position size ratio
                            risk_reward,
                            take_profit_ratio,
                            atr_spread, // spread by ATR
                            open_hours, // max open hours
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
        .filter(|(_, trading_strategy, _, _, _, _, _, _)| {
            strategy.is_none() || strategy == Some(trading_strategy)
        })
        .map(
            |(
                token,
                trading_strategy,
                amount,
                size_ratio,
                risk_reward,
                take_profit_ratio,
                atr_spread,
                open_hours,
            )| {
                (
                    token,
                    trading_strategy,
                    amount,
                    size_ratio,
                    risk_reward,
                    take_profit_ratio,
                    atr_spread,
                    *open_hours,
                )
            },
        )
        .collect()
}
