use debot_market_analyzer::TradingStrategy;

#[derive(PartialEq, Clone)]
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
    (u32, u32),
    f64,
    f64,
)> {
    let configs = vec![
        (
            "trend-follow-24h-1",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,     // h
            (0, 3), // activity time
            1.0,    // 1.0% (buy_signal_threshold)
            100.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-24h-2",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,     // h
            (3, 6), // activity time
            1.0,    // 1.0% (buy_signal_threshold)
            100.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-24h-3",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,     // h
            (6, 9), // activity time
            1.0,    // 1.0% (buy_signal_threshold)
            100.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-24h-4",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,      // h
            (9, 12), // activity time
            1.0,     // 1.0% (buy_signal_threshold)
            100.0,   // trading_amount(base token)
        ),
        (
            "trend-follow-24h-5",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,       // h
            (12, 15), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-24h-6",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,       // h
            (15, 18), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-24h-7",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,       // h
            (18, 21), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-24h-8",
            TradingStyle::Day,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            24,       // h
            (21, 24), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-30h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            30,     // h
            (0, 3), // activity time
            1.25,   // 1.25% (buy_signal_threshold)
            150.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-30h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            30,       // h
            (12, 15), // activity time
            1.25,     // 1.25% (buy_signal_threshold)
            150.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-36h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            36,     // h
            (3, 6), // activity time
            1.5,    // 1.5% (buy_signal_threshold)
            150.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-36h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            36,       // h
            (15, 18), // activity time
            1.5,      // 1.5% (buy_signal_threshold)
            150.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-42h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            42,     // h
            (6, 9), // activity time
            1.5,    // 1.5% (buy_signal_threshold)
            150.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-42h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            42,       // h
            (18, 21), // activity time
            1.5,      // 1.5% (buy_signal_threshold)
            150.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-48h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            48,      // h
            (9, 12), // activity time
            2.0,     // 2.0% (buy_signal_threshold)
            150.0,   // trading_amount(base token)
        ),
        (
            "trend-follow-48h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            48,       // h
            (21, 24), // activity time
            2.0,      // 2.0% (buy_signal_threshold)
            150.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-54h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            54,     // h
            (0, 3), // activity time
            1.0,    // 1.0% (buy_signal_threshold)
            100.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-54h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            54,       // h
            (12, 15), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-60h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            60,     // h
            (3, 6), // activity time
            1.0,    // 1.0% (buy_signal_threshold)
            100.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-60h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            60,       // h
            (15, 18), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-66h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            66,     // h
            (6, 9), // activity time
            1.0,    // 1.0% (buy_signal_threshold)
            100.0,  // trading_amount(base token)
        ),
        (
            "trend-follow-66h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            66,       // h
            (18, 21), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
        ),
        (
            "trend-follow-72h-1",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            72,      // h
            (9, 12), // activity time
            1.0,     // 1.0% (buy_signal_threshold)
            100.0,   // trading_amount(base token)
        ),
        (
            "trend-follow-72h-2",
            TradingStyle::Swing,
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            72,       // h
            (21, 24), // activity time
            1.0,      // 1.0% (buy_signal_threshold)
            100.0,    // trading_amount(base token)
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
                activity_time,
                buy_signal,
                trading_amount,
            )| {
                if config_trading_style == trading_style && config_chain_name == chain_name {
                    Some((
                        name.to_owned(),
                        config_trading_style.to_string(),
                        token_name.to_owned(),
                        strategy,
                        period,
                        activity_time,
                        buy_signal,
                        trading_amount,
                    ))
                } else {
                    None
                }
            },
        )
        .collect()
}
