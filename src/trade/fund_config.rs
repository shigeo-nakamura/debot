use debot_market_analyzer::TradingStrategy;

pub fn get() -> Vec<(String, TradingStrategy, f64, f64)> {
    vec![
        (
            "BTC-USDC".to_owned(),
            TradingStrategy::TrendFollowing,
            200.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "ETH-USDC".to_owned(),
            TradingStrategy::TrendFollowing,
            200.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "SOL-USDC".to_owned(),
            TradingStrategy::TrendFollowing,
            200.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "XRP-USDC".to_owned(),
            TradingStrategy::TrendFollowing,
            200.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "BNB-USDC".to_owned(),
            TradingStrategy::TrendFollowing,
            200.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
        (
            "LINK-USDC".to_owned(),
            TradingStrategy::TrendFollowing,
            200.0, // initial amount(in USDC)
            100.0, // amount(in USDC) per trading
        ),
    ]
}
