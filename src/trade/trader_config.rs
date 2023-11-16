use super::derivative_trader::SampleInterval;

pub fn get() -> (usize, SampleInterval, String) {
    (5, SampleInterval::new(1, 3), "mufex".to_owned())
}
