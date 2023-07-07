use super::TradingStrategy;

pub fn get(
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
) -> Vec<(&'static str, TradingStrategy, usize, f64, f64, f64, f64)> {
    vec![
        (
            "trend-follow-short",
            TradingStrategy::TrendFollowing,
            short_trade_period,
            1.01,  // buy_signal_threshold
            1.005, // take_profilt_threshold
            0.95,  // loss_cut_threshold
            10.0,  // initial_score
        ),
        (
            "trend-follow-medium",
            TradingStrategy::TrendFollowing,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.01, // take_profilt_threshold
            0.95, // loss_cut_threshold
            10.0, // initial_score
        ),
        (
            "trend-follow-long",
            TradingStrategy::TrendFollowing,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.02, // take_profilt_threshold
            0.95, // loss_cut_threshold
            10.0, // initial_score
        ),
        (
            "mean-reversion-medium",
            TradingStrategy::MeanReversion,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.01, // take_profilt_threshold
            0.95, // loss_cut_threshold
            10.0, // initial_score
        ),
        (
            "constrarian-medium",
            TradingStrategy::Contrarian,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.01, // take_profilt_threshold
            0.95, // loss_cut_threshold
            10.0, // initial_score
        ),
        (
            "ml-sdg-short",
            TradingStrategy::MLSGDPredictive,
            short_trade_period,
            1.01,  // buy_signal_threshold
            1.005, // take_profilt_threshold
            0.95,  // loss_cut_threshold
            10.0,  // initial_score
        ),
        (
            "ml-sdg-medium",
            TradingStrategy::MLSGDPredictive,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.01, // take_profilt_threshold
            0.95, // loss_cut_threshold
            10.0, // initial_score
        ),
        (
            "ml-sdg-long",
            TradingStrategy::MLSGDPredictive,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.02, // take_profilt_threshold
            0.95, // loss_cut_threshold
            10.0, // initial_score
        ),
    ]
}
