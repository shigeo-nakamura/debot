use crate::token_manager::{
    ChainParams, BSC_CHAIN_PARAMS, POLYGON_CHAIN_PARAMS, TESTNET_BSC_CHAIN_PARAMS,
    TESTNET_POLYGON_CHAIN_PARAMS,
};
use std::env;

pub struct EnvConfig {
    pub chain_params: &'static ChainParams,
    pub interval: u64,
    pub amount: f64,
    pub allowance_factor: f64,
    pub deadline_secs: u64,
    pub log_limit: usize,
}

pub fn get_config_from_env() -> Result<EnvConfig, &'static str> {
    let chain_name = env::var("CHAIN_NAME").unwrap_or_else(|_| "BSC_TESTNET".to_string());

    let chain_params: &'static ChainParams = match chain_name.as_str() {
        "BSC" => &BSC_CHAIN_PARAMS,
        "BSC_TESTNET" => &TESTNET_BSC_CHAIN_PARAMS,
        "POLYGON" => &POLYGON_CHAIN_PARAMS,
        "POLYGON_TESTNET" => &TESTNET_POLYGON_CHAIN_PARAMS,
        _ => return Err("Unsupported chain name"),
    };

    let interval_str = env::var("INTERVAL").unwrap_or_else(|_| "5".to_string());
    let interval = interval_str.parse::<u64>().unwrap();

    let amount_str = env::var("AMOUNT").unwrap_or_else(|_| "100.0".to_string());
    let amount = amount_str.parse::<f64>().unwrap();

    let allowance_factor_str =
        env::var("ALLOWANCE_FACTOR").unwrap_or_else(|_| "10000000000.0".to_string());
    let allowance_factor = allowance_factor_str.parse::<f64>().unwrap();

    let deadline_secs_str = env::var("DEADLINE_SECS").unwrap_or_else(|_| "60".to_string());
    let deadline_secs = deadline_secs_str.parse::<u64>().unwrap();

    let log_limit_str = env::var("LOG_LIMIT").unwrap_or_else(|_| "10000".to_string());
    let log_limit = log_limit_str.parse::<usize>().unwrap();

    Ok(EnvConfig {
        chain_params,
        interval,
        amount,
        allowance_factor,
        deadline_secs,
        log_limit,
    })
}
