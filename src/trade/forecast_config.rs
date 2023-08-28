use super::forecast_trader::TradingPeriod;

pub fn get() -> Vec<TradingPeriod> {
    let configs = vec![
        TradingPeriod::new(1, 4, 24),
        TradingPeriod::new(24, 120, 216),
    ];
    configs
}
