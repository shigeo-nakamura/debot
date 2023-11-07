use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 9;

pub const TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = [
    "BTC-USDC",
    "ETH-USDC",
    "SOL-USDC",
    "MATIC-USDC",
    "1000PEPE-USDC",
    "LINK-USDC",
    "LTC-USDC",
    "DOGE-USDC",
    "XRP-USDC",
];

pub fn get() -> Vec<(String, TradingStrategy, f64, f64)> {
    vec![
        // (
        //     TOKEN_LIST[0].to_owned(), // BTC
        //     TradingStrategy::TrendFollowLong,
        //     2000.0, // initial amount(in USDC)
        //     400.0,  // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[0].to_owned(), // BTC
        //     TradingStrategy::TrendFollowShort,
        //     2000.0, // initial amount(in USDC)
        //     400.0,  // amount(in USDC) per trading
        // ),
        (
            TOKEN_LIST[0].to_owned(), // BTC
            TradingStrategy::TrendFollowReactive,
            4000.0, // initial amount(in USDC)
            400.0,  // amount(in USDC) per trading
        ),
        // (
        //     TOKEN_LIST[1].to_owned(), // ETH
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[2].to_owned(), // SOL
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[3].to_owned(), // MATIC
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[4].to_owned(), // PEPE
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[5].to_owned(), // LINK
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[6].to_owned(), // LTC
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[7].to_owned(), // DOGE
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
        // (
        //     TOKEN_LIST[8].to_owned(), // XRP
        //     TradingStrategy::TrendFollowingLong,
        //     0.0,   // initial amount(in USDC)
        //     100.0, // amount(in USDC) per trading
        // ),
    ]
}
