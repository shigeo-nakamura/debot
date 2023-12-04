use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 1;

pub const MUFEX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = ["BTC-USDT"];

pub fn get(dex_name: &str) -> Vec<(String, TradingStrategy, f64)> {
    if dex_name == "mufex" {
        vec![
            (
                MUFEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow,
                2500.0, // initial amount(in USDC)
            ),
            (
                MUFEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::MeanReversion,
                2500.0, // initial amount(in USDC)
            ),
        ]
    } else {
        panic!("Unsupported dex");
    }
}
