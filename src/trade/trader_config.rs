use super::derivative_trader::SampleInterval;

pub fn get() -> Vec<(usize, SampleInterval, String)> {
    let configs = vec![(1, SampleInterval::new(1, 3), "apex".to_owned())];
    configs
}
