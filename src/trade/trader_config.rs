use super::derivative_trader::TradingPeriod;

pub fn get() -> Vec<(TradingPeriod, String)> {
    let configs = vec![(TradingPeriod::new(1, 4, 24), "apex".to_owned())];
    configs
}