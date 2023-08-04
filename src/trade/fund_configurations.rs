use crate::trade::trade_position::TakeProfitStrategy;

use super::{CutLossStrategy, TradingStrategy};

pub fn get(
    chain_name: &str,
    short_trade_period: usize,
    medium_trade_period: usize,
    long_trade_period: usize,
) -> Vec<(
    &'static str,
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
            "trend-follow-short",
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1 hour
        ),
        (
            "trend-follow-short",
            "BSC",
            "BTCB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1 hour
        ),
        (
            "trend-follow-short",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1 hour
        ),
        (
            "trend-follow-short",
            "BSC",
            "CAKE",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1 hour
        ),
        (
            "trend-follow-short",
            "POLYGON",
            "WETH",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 01 hour
        ),
        (
            "trend-follow-short",
            "POLYGON",
            "WMATIC",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            1.0,  // Predict the price in 1 hour
        ),
        (
            "trend-follow-medium",
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "trend-follow-medium",
            "BSC",
            "BTCB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "trend-follow-medium",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "trend-follow-medium",
            "BSC",
            "CAKE",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "trend-follow-medium",
            "POLYGON",
            "WETH",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "trend-follow-medium",
            "POLYGON",
            "WMATIC",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "trend-follow-long",
            "BSC",
            "ETH",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            2.0,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "trend-follow-long",
            "BSC",
            "BTCB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            2.0,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "trend-follow-long",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            2.0,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "trend-follow-long",
            "BSC",
            "CAKE",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            2.0,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "trend-follow-long",
            "POLYGON",
            "WETH",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            2.0,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "trend-follow-long",
            "POLYGON",
            "WMATIC",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            2.0,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "reversion-medium",
            "BSC",
            "ETH",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "reversion-medium",
            "BSC",
            "BTCB",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "reversion-medium",
            "BSC",
            "WBNB",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "reversion-medium",
            "BSC",
            "CAKE",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "reversion-medium",
            "POLYGON",
            "WETH",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "reversion-medium",
            "POLYGON",
            "WMATIC",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            medium_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
            12.0, // Predict the price in 12 hour
        ),
        (
            "reversion-long",
            "BSC",
            "ETH",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.5,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "reversion-long",
            "BSC",
            "BTCB",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.5,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "reversion-long",
            "BSC",
            "WBNB",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.5,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "reversion-long",
            "BSC",
            "CAKE",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.5,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "reversion-long",
            "POLYGON",
            "WETH",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.5,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
        (
            "reversion-long",
            "POLYGON",
            "WMATIC",
            TradingStrategy::MeanReversion,
            TakeProfitStrategy::TrailingStop,
            CutLossStrategy::ATRStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            1.03, // take_profit_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.5,  // 1 day
            24.0, // Predict the price in 24 hour
        ),
    ];

    configs
        .into_iter()
        .filter_map(
            |(
                name,
                target_chain_name,
                token_name,
                strategy,
                take_profit_strategy,
                cut_loss_strategy,
                period,
                buy_signal,
                take_profit,
                cut_loss,
                score,
                days,
                hours,
            )| {
                if target_chain_name == chain_name {
                    Some((
                        name,
                        token_name,
                        strategy,
                        take_profit_strategy,
                        cut_loss_strategy,
                        period,
                        buy_signal,
                        take_profit,
                        cut_loss,
                        score,
                        days,
                        hours,
                    ))
                } else {
                    None
                }
            },
        )
        .collect()
}
