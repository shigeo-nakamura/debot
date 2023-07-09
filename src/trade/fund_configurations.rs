use crate::trade::trade_position::StopLossStrategy;

use super::TradingStrategy;

pub fn get(
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
) -> Vec<(
    &'static str,
    TradingStrategy,
    StopLossStrategy,
    usize,
    f64,
    f64,
    f64,
    f64,
    f64,
)> {
    vec![(
        "trend-follow-short",
        TradingStrategy::TrendFollowing,
        StopLossStrategy::FixedThreshold,
        short_trade_period,
        1.01,  // buy_signal_threshold
        1.005, // take_profilt_threshold
        0.95,  // loss_cut_threshold
        10.0,  // initial_score
        1.0,   // 1 day
    )]
}
