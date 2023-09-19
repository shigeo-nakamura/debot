use super::TradingStrategy;

#[derive(PartialEq)]
pub enum TradingStyle {
    Day,
    Swing,
}

impl ToString for TradingStyle {
    fn to_string(&self) -> String {
        match self {
            TradingStyle::Day => String::from("Day"),
            TradingStyle::Swing => String::from("Swing"),
        }
    }
}

pub fn get(
    trading_style: TradingStyle,
    chain_name: &str,
) -> Vec<(
    String,
    String,
    String,
    TradingStrategy,
    usize,
    f64,
    f64,
    f64,
)> {
    let configs = vec![
        (
            "trend-follow-18h",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            18,    // h
            1.01,  // buy_signal_threshold
            100.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-18h",
            TradingStyle::Day,
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            18,    // h
            1.01,  // buy_signal_threshold
            100.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-24h",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,    // h
            1.012, // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-30h",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            30,    // h
            1.015, // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-36h",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            36,    // h
            1.02,  // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-42h",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            42,    // h
            1.025, // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-48h",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            48,    // h
            1.03,  // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-24h",
            TradingStyle::Swing,
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            24,    // h
            1.012, // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-30h",
            TradingStyle::Swing,
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            30,    // h
            1.015, // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-36h",
            TradingStyle::Swing,
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            36,    // h
            1.02,  // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-42h",
            TradingStyle::Swing,
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            42,    // h
            1.025, // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
        (
            "trend-follow-48h",
            TradingStyle::Swing,
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            48,    // h
            1.03,  // buy_signal_threshold
            200.0, // trading_amount(base token)
            10.0,  // initial_score
        ),
    ];

    configs
        .into_iter()
        .filter_map(
            |(
                name,
                config_trading_style,
                config_chain_name,
                token_name,
                strategy,
                period,
                buy_signal,
                trading_amount,
                score,
            )| {
                if config_trading_style == trading_style && config_chain_name == chain_name {
                    Some((
                        name.to_owned(),
                        config_trading_style.to_string(),
                        token_name.to_owned(),
                        strategy,
                        period,
                        buy_signal,
                        trading_amount,
                        score,
                    ))
                } else {
                    None
                }
            },
        )
        .collect()
}
