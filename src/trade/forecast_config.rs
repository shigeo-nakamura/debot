use super::forecast_trader::TradingPeriod;

pub fn get() -> Vec<(TradingPeriod, f64)> {
    let configs = vec![
        (TradingPeriod::new(1, 4, 24), 0.16),
        (TradingPeriod::new(24, 120, 216), 0.84),
    ];
    configs
}
