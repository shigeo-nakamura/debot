use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 1;

pub const RABBITX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = [
    "BTC-USD",
    // "ETH-USD",
    // "SOL-USD",
];

pub fn get(dex_name: &str) -> Vec<(String, TradingStrategy, f64)> {
    if dex_name == "rabbitx" {
        vec![
            (
                RABBITX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow,
                2500.0, // initial amount(in USD)
            ),
            // (
            //     RABBITX_TOKEN_LIST[1].to_owned(), // ETH
            //     TradingStrategy::TrendFollow,
            //     2500.0, // initial amount(in USD)
            // ),
            // (
            //     RABBITX_TOKEN_LIST[2].to_owned(), // SOL
            //     TradingStrategy::TrendFollow,
            //     2500.0, // initial amount(in USD)
            // ),
        ]
    } else {
        panic!("Unsupported dex");
    }
}
