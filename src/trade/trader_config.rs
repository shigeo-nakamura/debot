use super::derivative_trader::TradingPeriod;

pub fn get() -> Vec<(TradingPeriod, String)> {
    let configs = vec![(TradingPeriod::new(1, 5, 60), "apex".to_owned())];
    configs
}
