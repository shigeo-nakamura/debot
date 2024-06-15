use std::env;

use debot_market_analyzer::{TradingStrategy, TrendType};

use super::derivative_trader::SampleInterval;

pub fn get(strategy: Option<&TradingStrategy>) -> Vec<(usize, SampleInterval, String)> {
    let dex_name = env::var("DEX_NAME").expect("DEX_NAME must be specified");

    vec![
        (
            TradingStrategy::RandomWalk(TrendType::Unknown),
            24 * 60,
            SampleInterval::new(12 * 60, 26 * 60),
            dex_name.to_owned(),
        ),
        (
            TradingStrategy::MachineLearning(TrendType::Unknown),
            24 * 60,
            SampleInterval::new(12 * 60, 26 * 60),
            dex_name.to_owned(),
        ),
    ]
    .into_iter()
    .filter(|(trading_strategy, _, _, _)| match strategy {
        Some(strategy) => strategy == trading_strategy,
        None => false,
    })
    .map(
        |(_trading_strategy, trading_interval, interval, dex_name)| {
            (trading_interval, interval, dex_name)
        },
    )
    .collect()
}
