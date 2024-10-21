use debot_market_analyzer::{SampleTerm, TradingStrategy, TrendType};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST_SIZE: u32 = 1;
pub const TOKEN_LIST: &[&str] = &["BTC-USD"];

pub const CUT_LOSS_MIN_RATIO: f64 = 0.01;

lazy_static! {
    static ref INITIAL_FUND_AMOUNT: Decimal = env::var("INITIAL_FUND_AMOUNT")
        .ok()
        .and_then(|val| val.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::ZERO);
}

pub fn get(
    dex_name: &str,
    strategy: &TradingStrategy,
    leverage: u32,
) -> Vec<(
    String,
    TradingStrategy,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
    Option<Decimal>,
    SampleTerm,
    i64,
)> {
    let atr_term_values = vec![
        SampleTerm::TradingTerm,
        SampleTerm::ShortTerm,
        SampleTerm::LongTerm,
    ];

    let take_profit_ratio_values_random = vec![
        Some(Decimal::new(5, 3)),
        Some(Decimal::new(75, 4)),
        Some(Decimal::new(10, 3)),
        Some(Decimal::new(125, 4)),
        Some(Decimal::new(15, 3)),
        Some(Decimal::new(20, 3)),
        Some(Decimal::new(25, 3)),
        Some(Decimal::new(30, 3)),
    ];

    let take_profit_ratio_values_default = vec![
        Some(Decimal::new(5, 3)),
        Some(Decimal::new(75, 4)),
        Some(Decimal::new(10, 3)),
        Some(Decimal::new(125, 4)),
        Some(Decimal::new(15, 3)),
        Some(Decimal::new(20, 3)),
    ];

    let risk_reward_values = vec![Decimal::ONE];

    let atr_spread_values_random = vec![
        None,
        Some(Decimal::new(100, 3)),
        Some(Decimal::new(200, 3)),
        Some(Decimal::new(300, 3)),
        Some(Decimal::new(400, 3)),
        Some(Decimal::new(500, 3)),
        Some(Decimal::new(600, 3)),
        Some(Decimal::new(700, 3)),
        Some(Decimal::new(800, 3)),
        Some(Decimal::new(900, 3)),
        Some(Decimal::new(1000, 3)),
    ];

    let atr_spread_values_meanreversion = vec![
        Some(Decimal::new(200, 3)),
        Some(Decimal::new(400, 3)),
        Some(Decimal::new(800, 3)),
    ];

    let atr_spread_values_trendfollow = vec![None];

    let open_hours_values = vec![3, 6, 12, 24];

    let mut strategy_list = Vec::new();

    if dex_name == "hyperliquid" {
        let (take_profit_ratio_values, atr_spread_values, risk_reward_values, open_hours_values) =
            match strategy {
                TradingStrategy::RandomWalk(_) => (
                    take_profit_ratio_values_random,
                    atr_spread_values_random,
                    risk_reward_values,
                    open_hours_values,
                ),
                TradingStrategy::MeanReversion(_) => (
                    take_profit_ratio_values_default,
                    atr_spread_values_meanreversion,
                    risk_reward_values,
                    open_hours_values,
                ),
                TradingStrategy::TrendFollow(_) => (
                    take_profit_ratio_values_default,
                    atr_spread_values_trendfollow,
                    risk_reward_values,
                    open_hours_values,
                ),
            };

        let strategies = vec![
            TradingStrategy::RandomWalk(TrendType::Up),
            TradingStrategy::RandomWalk(TrendType::Down),
            TradingStrategy::MeanReversion(TrendType::Up),
            TradingStrategy::MeanReversion(TrendType::Down),
            TradingStrategy::TrendFollow(TrendType::Up),
            TradingStrategy::TrendFollow(TrendType::Down),
        ];

        for atr_term in &atr_term_values {
            for take_profit_ratio in take_profit_ratio_values.clone() {
                for atr_spread in atr_spread_values.clone() {
                    for risk_reward in risk_reward_values.clone() {
                        for open_hours in &open_hours_values {
                            for strategy in &strategies {
                                strategy_list.push((
                                    TOKEN_LIST[0].to_owned(),
                                    *strategy,
                                    Decimal::ZERO,
                                    Decimal::new(8, 1), // position size ratio
                                    risk_reward,
                                    take_profit_ratio,
                                    atr_spread,       // spread by ATR
                                    atr_term.clone(), // ATR SampleTerm
                                    *open_hours,      // max open hours
                                ));
                            }
                        }
                    }
                }
            }
        }
    } else {
        panic!("Unsupported dex");
    }

    // Filtered strategy list
    let filtered_strategy_list: Vec<_> = strategy_list
        .into_iter()
        .filter(|(_, trading_strategy, _, _, _, _, _, _, _)| strategy == trading_strategy)
        .collect();

    // Calculate the amount per strategy after filtering
    let filtered_strategies_count = filtered_strategy_list.len();
    let filtered_amount_per_strategy = if filtered_strategies_count > 0 {
        let initial_amount = *INITIAL_FUND_AMOUNT * Decimal::from(leverage);
        initial_amount / Decimal::from(filtered_strategies_count as u64)
    } else {
        panic!("No strategies found after filtering");
    };

    log::warn!("amount_per_strategy = {}", filtered_amount_per_strategy);

    // Update the amount for each filtered strategy
    filtered_strategy_list
        .into_iter()
        .map(
            |(
                token,
                trading_strategy,
                _,
                size_ratio,
                risk_reward,
                take_profit_ratio,
                atr_spread,
                atr_term,
                open_hours,
            )| {
                (
                    token,
                    trading_strategy,
                    filtered_amount_per_strategy, // Updated amount per strategy
                    size_ratio,
                    risk_reward,
                    take_profit_ratio,
                    atr_spread,
                    atr_term,
                    open_hours,
                )
            },
        )
        .collect()
}
