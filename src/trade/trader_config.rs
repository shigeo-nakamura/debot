use std::env;

use super::derivative_trader::SampleInterval;

pub fn get() -> (usize, SampleInterval, String, Option<usize>) {
    let dex_name = env::var("DEX_NAME").expect("DEX_NAME must be specified");
    (
        60,
        SampleInterval::new(240, 720),
        dex_name.to_owned(),
        Some(1),
    )
}
