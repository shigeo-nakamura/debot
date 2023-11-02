use super::derivative_trader::TradingPeriod;

pub fn get() -> Vec<(TradingPeriod, String)> {
    let configs = vec![(TradingPeriod::new(1, 2, 3), "apex".to_owned())];
    configs
}
