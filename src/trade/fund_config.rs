use debot_market_analyzer::TradingStrategy;

pub fn get() -> Vec<(String, TradingStrategy, f64, f64)> {
    vec![
        (
            "BTC-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            500.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "BTC-USDC".to_owned(),
            TradingStrategy::TrendFollowingShort,
            500.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "ETH-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "SOL-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "MATIC-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "1000PEPE-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "LINK-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "LTC-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "DOGE-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "XRP-USDC".to_owned(),
            TradingStrategy::TrendFollowingLong,
            0.0,   // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
    ]
}
