use debot_market_analyzer::{SampleTerm, TradingStrategy, TrendType};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use std::env;

pub const TOKEN_LIST_SIZE: u32 = 1;

pub const TOKEN_LIST: &[&str] = &["BTC-USD"];

lazy_static! {
    static ref INITIAL_FUND_AMOUNT: Decimal = env::var("INITIAL_FUND_AMOUNT")
        .ok()
        .and_then(|val| val.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(50, 0));
}

pub fn get(
    dex_name: &str,
    strategy: Option<&TradingStrategy>,
    interval_secs: i64,
) -> Vec<(
    String,
    TradingStrategy,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
    Option<Decimal>,
    SampleTerm,
    u32,
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

    let take_profit_ratio_values_meanreversion = vec![
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
        None,
        Some(Decimal::new(200, 3)),
        Some(Decimal::new(400, 3)),
        Some(Decimal::new(800, 3)),
    ];

    let open_hours_values = vec![3, 6, 12, 24];

    let mut strategy_list = Vec::new();

    let initial_amount = *INITIAL_FUND_AMOUNT;

    if dex_name == "hyperliquid" {
        let (take_profit_ratio_values, atr_spread_values, risk_reward_values, open_hours_values) =
            match strategy {
                Some(TradingStrategy::RandomWalk(_)) => (
                    take_profit_ratio_values_random,
                    atr_spread_values_random,
                    risk_reward_values,
                    open_hours_values,
                ),
                Some(TradingStrategy::MeanReversion(_)) | None => (
                    take_profit_ratio_values_meanreversion,
                    atr_spread_values_meanreversion,
                    risk_reward_values,
                    open_hours_values,
                ),
            };

        for atr_term in &atr_term_values {
            for take_profit_ratio in take_profit_ratio_values.clone() {
                for atr_spread in atr_spread_values.clone() {
                    for risk_reward in risk_reward_values.clone() {
                        for open_hours in &open_hours_values {
                            let open_tick_count_max: u32 =
                                (open_hours * 60 * 60 / interval_secs).try_into().unwrap();
                            strategy_list.push((
                                TOKEN_LIST[0].to_owned(), // BTC
                                TradingStrategy::RandomWalk(TrendType::Up),
                                initial_amount,     // initial amount (in USD)
                                Decimal::new(8, 1), // position size ratio
                                risk_reward,
                                take_profit_ratio,
                                atr_spread,          // spread by ATR
                                atr_term.clone(),    // ATR SampleTerm
                                open_tick_count_max, // max open tick count
                            ));

                            strategy_list.push((
                                TOKEN_LIST[0].to_owned(), // BTC
                                TradingStrategy::RandomWalk(TrendType::Down),
                                initial_amount,     // initial amount (in USD)
                                Decimal::new(8, 1), // position size ratio
                                risk_reward,
                                take_profit_ratio,
                                atr_spread,          // spread by ATR
                                atr_term.clone(),    // ATR SampleTerm
                                open_tick_count_max, // max open tick count
                            ));

                            strategy_list.push((
                                TOKEN_LIST[0].to_owned(), // BTC
                                TradingStrategy::MeanReversion(TrendType::Up),
                                initial_amount,     // initial amount (in USD)
                                Decimal::new(8, 1), // position size ratio
                                risk_reward,
                                take_profit_ratio,
                                atr_spread,          // spread by ATR
                                atr_term.clone(),    // ATR SampleTerm
                                open_tick_count_max, // max open tick count
                            ));

                            strategy_list.push((
                                TOKEN_LIST[0].to_owned(), // BTC
                                TradingStrategy::MeanReversion(TrendType::Down),
                                initial_amount,     // initial amount (in USD)
                                Decimal::new(8, 1), // position size ratio
                                risk_reward,
                                take_profit_ratio,
                                atr_spread,          // spread by ATR
                                atr_term.clone(),    // ATR SampleTerm
                                open_tick_count_max, // max open tick count
                            ));
                        }
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
        .filter(|(_, trading_strategy, _, _, _, _, _, _, _)| {
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
                atr_term,
                open_tick_count_max,
            )| {
                (
                    token,
                    trading_strategy,
                    amount,
                    size_ratio,
                    risk_reward,
                    take_profit_ratio,
                    atr_spread,
                    atr_term,
                    open_tick_count_max,
                )
            },
        )
        .collect()
}

pub fn get_vectors_and_count(
    dex_name: &str,
    strategy: &TradingStrategy,
    interval_secs: i64,
) -> (
    Vec<Decimal>,    // take_profit_ratio
    Vec<Decimal>,    // atr_spread
    Vec<u32>,        // open_tick_count_max
    Vec<SampleTerm>, // atr_term
    usize,           // vector count
) {
    let strategies = get(dex_name, None, interval_secs);

    let mut take_profit_ratio = Vec::new();
    let mut atr_spread = Vec::new();
    let mut open_tick_count_max = Vec::new();
    let mut atr_term = Vec::new();

    for (
        _token,
        _trading_strategy,
        _amount,
        _size_ratio,
        _risk_reward,
        take_profit,
        atr,
        atr_t,
        open_tick,
    ) in strategies
        .into_iter()
        .filter(|(_, trading_strategy, _, _, _, _, _, _, _)| trading_strategy == strategy)
    {
        take_profit_ratio.push(take_profit.unwrap_or_default());
        atr_spread.push(atr.unwrap_or_default());
        open_tick_count_max.push(open_tick);
        atr_term.push(atr_t);
    }

    let vector_count = take_profit_ratio.len();

    (
        take_profit_ratio,
        atr_spread,
        open_tick_count_max,
        atr_term,
        vector_count,
    )
}
