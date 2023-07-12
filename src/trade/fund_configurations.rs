use crate::trade::trade_position::StopLossStrategy;

use super::TradingStrategy;

pub fn get(
    index: usize,
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
    f64,
)> {
    let configs = vec![
        vec![
            (
                "trend-follow-short-trailing",
                TradingStrategy::TrendFollowing,
                StopLossStrategy::TrailingStop,
                short_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.95, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                0.1,  // Predict the price in 0.1 hour
            ),
            (
                "trend-follow-medium-trailing",
                TradingStrategy::TrendFollowing,
                StopLossStrategy::TrailingStop,
                medium_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.95, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                1.0,  // Predict the price in 1.0 hour
            ),
            (
                "trend-follow-long-trailing",
                TradingStrategy::TrendFollowing,
                StopLossStrategy::TrailingStop,
                long_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.95, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                3.0,  // Predict the price in 3.0 hour
            ),
        ],
        vec![
            (
                "trend-follow-short-fixed",
                TradingStrategy::TrendFollowing,
                StopLossStrategy::FixedThreshold,
                short_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.95, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                0.1,  // Predict the price in 0.1 hour
            ),
            (
                "trend-follow-medium-fixed",
                TradingStrategy::TrendFollowing,
                StopLossStrategy::FixedThreshold,
                medium_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.95, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                1.0,  // Predict the price in 1.0 hour
            ),
            (
                "trend-follow-long-fixed",
                TradingStrategy::TrendFollowing,
                StopLossStrategy::FixedThreshold,
                long_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.95, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                3.0,  // Predict the price in 3.0 hour
            ),
        ],
    ];
    configs[index].clone()
}
