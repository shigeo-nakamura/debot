use std::env;

use debot_market_analyzer::TradingStrategy;

use super::derivative_trader::SampleInterval;

pub fn get(
    strategy: Option<&TradingStrategy>,
) -> Vec<(usize, SampleInterval, String, Option<usize>)> {
    let dex_name = env::var("DEX_NAME").expect("DEX_NAME must be specified");

    vec![
        (
            TradingStrategy::Rebalance,
            60,
            SampleInterval::new(30, 240),
            dex_name.to_owned(),
            Some(1),
        ),
        (
            TradingStrategy::MarketMake,
            3,
            SampleInterval::new(30, 240),
            dex_name.to_owned(),
            Some(3),
        ),
    ]
    .into_iter()
    .filter(|(trading_strategy, _, _, _, _)| {
        strategy.is_none() || strategy == Some(trading_strategy)
    })
    .map(
        |(trading_strategy, trading_interval, interval, dex_name, execution_interval)| {
            (trading_interval, interval, dex_name, execution_interval)
        },
    )
    .collect()
}
