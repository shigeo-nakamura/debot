use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 3;
pub const MUFEX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = ["BTC-USDT", "ETH-USDT", "SOL-USDT"];
pub const APEX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = ["BTC-USDC", "ETH-USDC", "SOL-USDC"];

pub fn get(dex_name: &str) -> Vec<(String, TradingStrategy, f64)> {
    if dex_name == "mufex" {
        vec![(
            MUFEX_TOKEN_LIST[0].to_owned(), // BTC
            TradingStrategy::TrendFollow,
            2500.0, // initial amount(in USD)
        )]
    } else if dex_name == "apex" {
        vec![
            (
                APEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow,
                2500.0, // initial amount(in USD)
            ),
            (
                APEX_TOKEN_LIST[1].to_owned(), // ETH
                TradingStrategy::TrendFollow,
                2500.0, // initial amount(in USD)
            ),
            (
                APEX_TOKEN_LIST[2].to_owned(), // SOL
                TradingStrategy::TrendFollow,
                2500.0, // initial amount(in USD)
            ),
        ]
    } else {
        panic!("Unsupported dex");
    }
}
