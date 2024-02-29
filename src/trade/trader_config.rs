use std::env;

use super::derivative_trader::SampleInterval;

pub fn get() -> (usize, SampleInterval, String, Option<usize>) {
    let dex_name = env::var("DEX_NAME").expect("DEX_NAME must be specified");
    (1, SampleInterval::new(1, 10), dex_name.to_owned(), Some(60))
}
