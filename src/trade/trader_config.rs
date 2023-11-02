use super::derivative_trader::TradingPeriod;

pub fn get() -> Vec<(TradingPeriod, String)> {
    let configs = vec![(TradingPeriod::new(0.1, 0.4, 2.4), "apex".to_owned())];
    configs
}
