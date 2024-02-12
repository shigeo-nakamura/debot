use debot_market_analyzer::TradingStrategy;

pub const TOKEN_LIST_SIZE: usize = 6;

pub const RABBITX_TOKEN_LIST: [&str; TOKEN_LIST_SIZE] = [
    "BTC-USD", "ETH-USD", "SOL-USD", "SUI-USD", "APT-USD", "ARB-USD",
];

pub fn get(dex_name: &str, strategy: &TradingStrategy) -> Vec<(String, f64, f64, f64, f64)> {
    if dex_name == "rabbitx" {
        let all_funds = vec![
            (
                RABBITX_TOKEN_LIST[0].to_owned(), // BTC
                TradingStrategy::TrendFollow,
                500.0, // initial amount(in USD)
                0.02,  // position size ration (2%)
                1.2,   // risk reward
                0.001, // loss cut ration (0.1%)
            ),
            (
                RABBITX_TOKEN_LIST[1].to_owned(), // ETH
                TradingStrategy::TrendFollow,
                0.0,   // initial amount(in USD)
                0.02,  // position size ration (2%)
                1.2,   // risk reward
                0.001, // loss cut ration (0.1%)
            ),
            (
                RABBITX_TOKEN_LIST[2].to_owned(), // SOL
                TradingStrategy::TrendFollow,
                0.0,   // initial amount(in USD)
                0.02,  // position size ration (2%)
                1.2,   // risk reward
                0.001, // loss cut ration (0.1%)
            ),
            (
                RABBITX_TOKEN_LIST[3].to_owned(), // SUI
                TradingStrategy::TrendFollow,
                0.0,   // initial amount(in USD)
                0.02,  // position size ration (2%)
                1.2,   // risk reward
                0.001, // loss cut ration (0.1%)
            ),
            (
                RABBITX_TOKEN_LIST[4].to_owned(), // APT
                TradingStrategy::TrendFollow,
                0.0,   // initial amount(in USD)
                0.02,  // position size ration (2%)
                1.2,   // risk reward
                0.001, // loss cut ration (0.1%)
            ),
            (
                RABBITX_TOKEN_LIST[5].to_owned(), // ARB
                TradingStrategy::TrendFollow,
                0.0,   // initial amount(in USD)
                0.02,  // position size ration (2%)
                1.2,   // risk reward
                0.001, // loss cut ration (0.1%)
            ),
        ];

        all_funds
            .into_iter()
            .filter(|(_, token_strategy, _, _, _, _)| token_strategy == strategy)
            .map(
                |(token, _, amount, size_ratio, risk_reward, loss_cut_ratio)| {
                    (token, amount, size_ratio, risk_reward, loss_cut_ratio)
                },
            )
            .collect()
    } else {
        panic!("Unsupported dex");
    }
}
