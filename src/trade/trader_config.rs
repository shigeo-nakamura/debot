use std::env;

use super::derivative_trader::SampleInterval;

pub fn get() -> (usize, SampleInterval, String) {
    let dex_name = env::var("DEX_NAME").expect("DEX_NAME must be specified");
    (15, SampleInterval::new(5, 15), dex_name.to_owned())
}
