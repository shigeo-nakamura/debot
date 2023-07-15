use crate::trade::trade_position::TakeProfitStrategy;

use super::{CutLossStrategy, TradingStrategy};

pub fn get(
    index: usize,
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
) -> Vec<(
    &'static str,
    TradingStrategy,
    TakeProfitStrategy,
    CutLossStrategy,
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
                TakeProfitStrategy::TrailingStop,
                CutLossStrategy::ATRStop,
                short_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.99, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                0.1,  // Predict the price in 0.1 hour
            ),
            (
                "trend-follow-medium-trailing",
                TradingStrategy::TrendFollowing,
                TakeProfitStrategy::TrailingStop,
                CutLossStrategy::ATRStop,
                medium_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.99, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                1.0,  // Predict the price in 1.0 hour
            ),
            (
                "trend-follow-long-trailing",
                TradingStrategy::TrendFollowing,
                TakeProfitStrategy::TrailingStop,
                CutLossStrategy::ATRStop,
                long_trade_period,
                1.01, // buy_signal_threshold
                1.01, // take_profit_threshold
                0.99, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                3.0,  // Predict the price in 3.0 hour
            ),
        ],
        vec![
            (
                "trend-follow-short-fixed",
                TradingStrategy::TrendFollowing,
                TakeProfitStrategy::FixedThreshold,
                CutLossStrategy::FixedThreshold,
                short_trade_period,
                1.01, // buy_signal_threshold
                1.02, // take_profit_threshold
                0.99, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                0.1,  // Predict the price in 0.1 hour
            ),
            (
                "trend-follow-medium-fixed",
                TradingStrategy::TrendFollowing,
                TakeProfitStrategy::FixedThreshold,
                CutLossStrategy::FixedThreshold,
                medium_trade_period,
                1.01, // buy_signal_threshold
                1.02, // take_profit_threshold
                0.99, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                1.0,  // Predict the price in 1.0 hour
            ),
            (
                "trend-follow-long-fixed",
                TradingStrategy::TrendFollowing,
                TakeProfitStrategy::FixedThreshold,
                CutLossStrategy::FixedThreshold,
                long_trade_period,
                1.01, // buy_signal_threshold
                1.02, // take_profit_threshold
                0.99, // loss_cut_threshold
                10.0, // initial_score
                1.0,  // 1 day
                3.0,  // Predict the price in 3.0 hour
            ),
        ],
    ];
    configs[index].clone()
}
