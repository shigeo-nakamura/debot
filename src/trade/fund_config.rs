use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 1;

pub const APEX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = ["BTC-USDC"];
pub const MUFEX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = ["BTC-USDT"];

pub fn get(dex_name: &str) -> Vec<(String, TradingStrategy, f64, f64)> {
    if dex_name == "apex" {
        vec![
            (
                APEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollowLong,
                5000.0, // initial amount(in USDC)
                200.0,  // amount(in USDC) per trading
            ),
            (
                APEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollowShort,
                5000.0, // initial amount(in USDC)
                200.0,  // amount(in USDC) per trading
            ),
        ]
    } else {
        vec![
            (
                MUFEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollowLong,
                5000.0, // initial amount(in USDC)
                200.0,  // amount(in USDC) per trading
            ),
            (
                MUFEX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollowShort,
                5000.0, // initial amount(in USDC)
                200.0,  // amount(in USDC) per trading
            ),
        ]
    }
}
