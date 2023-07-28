use crate::trade::trade_position::TakeProfitStrategy;

use super::{CutLossStrategy, TradingStrategy};

pub fn get(
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
        (
            "trend-follow-short-trailing-atr",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            0.1,  // Predict the price in 0.1 hour
        ),
        (
            "trend-follow-medium-trailing-atr",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1.0 hour
        ),
        (
            "trend-follow-long-trailing-atr",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            3.0,  // Predict the price in 3.0 hour
        ),
        (
            "reversion-medium-trailing-atr",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1.0 hour
        ),
        (
            "reversion-long-trailing-atr",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            3.0,  // Predict the price in 3.0 hour
        ),
    ];
    configs
}
