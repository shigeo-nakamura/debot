use super::TradingStrategy;

pub fn get(
    chain_name: &str,
) -> Vec<(&'static str, &'static str, TradingStrategy, usize, f64, f64)> {
    let configs = vec![
        (
            "trend-follow-6h",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            6,    // h
            1.01, // buy_signal_threshold
            10.0, // initial_score
        ),
        (
            "trend-follow-12h",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            12,   // h
            1.01, // buy_signal_threshold
            10.0, // initial_score
        ),
        (
            "trend-follow-24h",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,   // h
            1.01, // buy_signal_threshold
            10.0, // initial_score
        ),
    ];

    configs
        .into_iter()
        .filter_map(
            |(name, target_chain_name, token_name, strategy, period, buy_signal, score)| {
                if target_chain_name == chain_name {
                    Some((name, token_name, strategy, period, buy_signal, score))
                } else {
                    None
                }
            },
        )
        .collect()
}
