use super::forecast_trader::TradingPeriod;

pub fn get() -> Vec<(TradingPeriod, f64)> {
    let configs = vec![
        (TradingPeriod::new(1, 4, 24), 0.4),
        // (TradingPeriod::new(12, 36, 72), 0.6),
    ];
    configs
}
