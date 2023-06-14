use crate::token_manager::{
    ChainParams, BSC_CHAIN_PARAMS, POLYGON_CHAIN_PARAMS, TESTNET_BSC_CHAIN_PARAMS,
    TESTNET_POLYGON_CHAIN_PARAMS,
};
use std::env;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

#[derive(Debug)]
pub struct EnvConfig {
    pub chain_params: &'static ChainParams,
    pub interval: u64,
    pub amount: f64,
    pub allowance_factor: f64,
    pub deadline_secs: u64,
    pub log_limit: usize,
    pub skip_write: bool,
    pub num_swaps: usize,
    pub short_trade_period: usize,
    pub long_trade_period: usize,
    pub loss_limit_ratio: f64,
    pub profit_limit_ratio: f64,
    pub max_position_amount: f64,
    pub max_hold_period: usize,
    pub match_multiplier: f64,
    pub mismatch_multiplier: f64,
}

#[derive(Debug)]
pub enum ConfigError {
    UnsupportedChainName,
    ParseIntError(ParseIntError),
    ParseFloatError(ParseFloatError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::UnsupportedChainName => write!(f, "Unsupported chain name"),
            ConfigError::ParseIntError(e) => write!(f, "Parse int error: {}", e),
            ConfigError::ParseFloatError(e) => write!(f, "Parse float error: {}", e),
        }
    }
}

impl From<ParseIntError> for ConfigError {
    fn from(err: ParseIntError) -> ConfigError {
        ConfigError::ParseIntError(err)
    }
}

impl From<ParseFloatError> for ConfigError {
    fn from(err: ParseFloatError) -> ConfigError {
        ConfigError::ParseFloatError(err)
    }
}

fn get_env_var<T: std::str::FromStr>(
    var: &str,
    default: &str,
) -> Result<T, <T as std::str::FromStr>::Err> {
    let var_str = env::var(var).unwrap_or_else(|_| default.to_string());
    var_str.parse::<T>()
}

fn get_bool_env_var(var: &str, default: bool) -> bool {
    match env::var(var) {
        Ok(val) => {
            let lower_val = val.to_lowercase();
            lower_val != "false" && lower_val != "0"
        }
        Err(_e) => {
            // Environment variable not found, use default value
            default
        }
    }
}

pub fn get_config_from_env() -> Result<Vec<EnvConfig>, ConfigError> {
    let chain_names = env::var("CHAIN_NAME").unwrap_or_else(|_| "BSC_TESTNET".to_string());
    let chain_names: Vec<&str> = chain_names.split(',').collect();

    log::debug!("{:?}", chain_names);

    let mut env_configs = vec![];
    for chain_name in chain_names {
        let chain_name = chain_name.trim(); // To handle spaces after the comma

        let chain_params: &'static ChainParams = match chain_name {
            "BSC" => &BSC_CHAIN_PARAMS,
            "BSC_TESTNET" => &TESTNET_BSC_CHAIN_PARAMS,
            "POLYGON" => &POLYGON_CHAIN_PARAMS,
            "POLYGON_TESTNET" => &TESTNET_POLYGON_CHAIN_PARAMS,
            _ => return Err(ConfigError::UnsupportedChainName),
        };

        let interval = get_env_var("INTERVAL", "60")?;
        let amount = get_env_var("AMOUNT", "100.0")?;
        let allowance_factor = get_env_var("ALLOWANCE_FACTOR", "10000000000.0")?;
        let deadline_secs = get_env_var("DEADLINE_SECS", "60")?;
        let log_limit = get_env_var("LOG_LIMIT", "10000")?;
        let skip_write = get_bool_env_var("SKIP_WRITE", true);
        let num_swaps = get_env_var("NUM_SWAPS", "3")?;
        let short_trade_period = get_env_var("SHORT_TRADE_PEREIOD", "60")?;
        let long_trade_period = get_env_var("LONG_TRACE_PEREIOD", "600")?;
        let loss_limit_ratio = get_env_var("LOSS_LIMIT_RATIO", "-1.0")?;
        let profit_limit_ratio = get_env_var("PROFIT_LIMIT_RATIO", "2.0")?;
        let max_position_amount = get_env_var("MAX_POSITION_AMOUNT", "500.0")?;
        let max_hold_period = get_env_var("MAX_HOLD_PERIOD", "300")?;
        let match_multiplier = get_env_var("MATCH_MULTIPLIER", "1.5")?;
        let mismatch_multiplier = get_env_var("MISMATCH_MULTIPLIER", "0.5")?;

        let env_config = EnvConfig {
            chain_params,
            interval,
            amount,
            allowance_factor,
            deadline_secs,
            log_limit,
            skip_write,
            num_swaps,
            short_trade_period,
            long_trade_period,
            loss_limit_ratio,
            profit_limit_ratio,
            max_position_amount,
            max_hold_period,
            match_multiplier,
            mismatch_multiplier,
        };

        env_configs.push(env_config);
    }

    Ok(env_configs)
}
