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
    pub use_kms: bool,
    pub interval: u64,
    pub amount: f64,
    pub min_initial_amount: f64,
    pub max_position_amount: f64,
    pub allowance_factor: f64,
    pub deadline_secs: u64,
    pub log_limit: usize,
    pub skip_write: bool,
    pub num_swaps: usize,
    pub short_trade_period: usize,
    pub medium_trade_period: usize,
    pub long_trade_period: usize,
    pub max_hold_period: usize,
    pub percentage_loss_threshold: f64,
    pub percentage_profit_threshold: f64,
    pub percentage_drop_threshold: f64,
    pub max_error_count: u32,
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
            lower_val == "true" || lower_val == "1"
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

        let use_kms = get_bool_env_var("USE_KMS", false);
        let interval = get_env_var("INTERVAL", "10")?;
        let amount = get_env_var("AMOUNT", "100.0")?;
        let min_initial_amount = get_env_var("MIN_INITIAL_AMOUNT", "500.0")?;
        let max_position_amount = get_env_var("MAX_POSITION_AMOUNT", "500.0")?;
        let allowance_factor = get_env_var("ALLOWANCE_FACTOR", "10000000000.0")?;
        let deadline_secs = get_env_var("DEADLINE_SECS", "60")?;
        let log_limit = get_env_var("LOG_LIMIT", "10000")?;
        let skip_write = get_bool_env_var("SKIP_WRITE", true);
        let num_swaps = get_env_var("NUM_SWAPS", "3")?;
        let short_trade_period = get_env_var("SHORT_TRADE_PEREIOD", "100")?;
        let medium_trade_period = get_env_var("MEDIUM_TRADE_PEREIOD", "1000")?;
        let long_trade_period = get_env_var("LONG_TRACE_PEREIOD", "10000")?;
        let max_hold_period = get_env_var("MAX_HOLD_PERIOD", "300")?;
        let percentage_loss_threshold = get_env_var("PERCENTAGE_LOSS_THRESHOLD", "-1.0")?;
        let percentage_profit_threshold = get_env_var("PERCENTAGE_PROFIT_THRESHOLD", "1.0")?;
        let percentage_drop_threshold = get_env_var("PERCENTAGE_DROP_THRESHOLD", "3.0")?;
        let max_error_count = get_env_var("MAX_ERROR_COUNT", "3")?;
        let match_multiplier = get_env_var("MATCH_MULTIPLIER", "1.2")?;
        let mismatch_multiplier = get_env_var("MISMATCH_MULTIPLIER", "0.5")?;

        let env_config = EnvConfig {
            chain_params,
            use_kms,
            interval,
            amount,
            min_initial_amount,
            max_position_amount,
            allowance_factor,
            deadline_secs,
            log_limit,
            skip_write,
            num_swaps,
            short_trade_period,
            medium_trade_period,
            long_trade_period,
            max_hold_period,
            percentage_loss_threshold,
            percentage_profit_threshold,
            percentage_drop_threshold,
            max_error_count,
            match_multiplier,
            mismatch_multiplier,
        };

        env_configs.push(env_config);
    }

    Ok(env_configs)
}
