use super::TradingStrategy;

pub fn get(
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
) -> Vec<(&'static str, TradingStrategy, usize, f64, f64, f64)> {
    vec![
        (
            "trend-follow-short",
            TradingStrategy::TrendFollowing,
            short_trade_period,
            1.01,
            0.95,
            10.0,
        ),
        (
            "trend-follow-medium",
            TradingStrategy::TrendFollowing,
            medium_trade_period,
            1.01,
            0.95,
            10.0,
        ),
        (
            "trend-follow-long",
            TradingStrategy::TrendFollowing,
            long_trade_period,
            1.01,
            0.95,
            10.0,
        ),
        (
            "mean-reversion-medium",
            TradingStrategy::MeanReversion,
            medium_trade_period,
            1.01,
            0.95,
            10.0,
        ),
        (
            "constrarian-medium",
            TradingStrategy::Contrarian,
            medium_trade_period,
            1.01,
            0.95,
            10.0,
        ),
        (
            "ml-sdg-short",
            TradingStrategy::MLSGDPredictive,
            short_trade_period,
            1.01,
            0.95,
            0.0,
        ),
        (
            "ml-sdg-medium",
            TradingStrategy::MLSGDPredictive,
            medium_trade_period,
            1.01,
            0.95,
            0.0,
        ),
        (
            "ml-sdg-long",
            TradingStrategy::MLSGDPredictive,
            long_trade_period,
            1.01,
            0.95,
            0.0,
        ),
    ]
}
