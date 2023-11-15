use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 1;

pub const TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = ["BTC-USDT"];

pub fn get() -> Vec<(String, TradingStrategy, f64, f64)> {
    vec![
        (
            TOKEN_LIST[0].to_owned(), // BTC
            TradingStrategy::TrendFollowLong,
            2000.0, // initial amount(in USDC)
            400.0,  // amount(in USDC) per trading
        ),
        (
            TOKEN_LIST[0].to_owned(), // BTC
            TradingStrategy::TrendFollowShort,
            2000.0, // initial amount(in USDC)
            400.0,  // amount(in USDC) per trading
        ),
        // (
        //     TOKEN_LIST[0].to_owned(), // BTC
        //     TradingStrategy::TrendFollowReactive,
        //     4000.0, // initial amount(in USDC)
        //     400.0,  // amount(in USDC) per trading
        // ),
    ]
}
