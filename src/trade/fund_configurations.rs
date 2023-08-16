use super::TradingStrategy;
use crate::trade::trade_position::TakeProfitStrategy;

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
    usize,
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
            "WBNB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            short_trade_period,
            1.01, // buy_signal_threshold
            0.99, // loss_cut_threshold
            10.0, // initial_score
            0.05, // 1 day
            1.0,  // Predict the price in 1 hour
        ),
        (
            "trend-follow-medium",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            medium_trade_period,
            1.01,  // buy_signal_threshold
            0.985, // loss_cut_threshold
            10.0,  // initial_score
            0.5,   // 0.5 day
            12.0,  // Predict the price in 12 hour
        ),
        (
            "trend-follow-long",
            "BSC",
            "WBNB",
            TradingStrategy::TrendFollowing,
            TakeProfitStrategy::TrailingStop,
            long_trade_period,
            1.01, // buy_signal_threshold
            0.98, // loss_cut_threshold
            10.0, // initial_score
            1.0,  // 1 day
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
