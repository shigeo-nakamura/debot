use std::env;

use debot_market_analyzer::{TradingStrategy, TrendType};

use super::derivative_trader::SampleInterval;

pub fn get(strategy: Option<&TradingStrategy>) -> Vec<(usize, SampleInterval, String)> {
    let dex_name = env::var("DEX_NAME").expect("DEX_NAME must be specified");

    vec![
        (
            TradingStrategy::TrendFollow(TrendType::Unknown),
            60,
            SampleInterval::new(30, 240),
            dex_name.to_owned(),
        ),
        (
            TradingStrategy::MeanReversion(TrendType::Unknown),
            5,
            SampleInterval::new(12, 26),
            dex_name.to_owned(),
        ),
    ]
    .into_iter()
    .filter(|(trading_strategy, _, _, _)| match strategy {
        Some(strategy) => {
            strategy == trading_strategy
                || matches!(trading_strategy, TradingStrategy::PassiveTrade(_))
        }
        None => true,
    })
    .map(
        |(_trading_strategy, trading_interval, interval, dex_name)| {
            (trading_interval, interval, dex_name)
        },
    )
    .collect()
}
