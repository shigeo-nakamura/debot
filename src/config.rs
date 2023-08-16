use ethers::abi::Address;

use crate::blockchain_factory::{
    ChainParams, BSC_CHAIN_PARAMS, POLYGON_CHAIN_PARAMS, TESTNET_BSC_CHAIN_PARAMS,
    TESTNET_POLYGON_CHAIN_PARAMS,
};
use std::env;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use std::str::FromStr;

#[derive(Debug)]
pub struct EnvConfig {
    pub chain_params: &'static ChainParams,
    pub rpc_node_index: usize,
    pub mongodb_uri: String,
    pub db_name: String,
    pub use_kms: bool,
    pub interval: u64,
    pub min_managed_amount: f64,
    pub max_managed_amount: f64,
    pub trading_amount: f64,
    pub allowance_factor: f64,
    pub deadline_secs: u64,
    pub log_limit: u32,
    pub dry_run: bool,
    pub num_swaps: usize,
    pub short_trade_period: usize,
    pub medium_trade_period: usize,
    pub long_trade_period: usize,
    pub max_price_size: u32,
    pub position_creation_inteval_seconds: Option<u64>,
    pub risk_reward: f64,
    pub max_error_count: u32,
    pub reward_multiplier: f64,
    pub penalty_multiplier: f64,
    pub relative_spread: f64,
    pub save_prices: bool,
    pub treasury: Option<Address>,
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
    let chain_names = env::var("CHAIN_NAME").unwrap_or_else(|_| "BSC".to_string());
    let chain_names: Vec<&str> = chain_names.split(',').collect();
    let mut env_configs = vec![];

    let mut polygon_index = 0;
    let mut bsc_index = 0;

    for chain_name in chain_names {
        let mut rpc_node_index = 0;
        let mut dry_run = false;

        let chain_name = chain_name.trim(); // To handle spaces after the comma
        let chain_params: &'static ChainParams = match chain_name {
            "BSC" => {
                dry_run = get_bool_env_var("BSC_DRY_RUN", true);
                rpc_node_index = bsc_index;
                bsc_index += 1;
                &BSC_CHAIN_PARAMS
            }
            "POLYGON" => {
                dry_run = get_bool_env_var("POLYGON_DRY_RUN", true);
                rpc_node_index = polygon_index;
                polygon_index += 1;
                &POLYGON_CHAIN_PARAMS
            }
            "BSC_TESTNET" => &TESTNET_BSC_CHAIN_PARAMS,
            "POLYGON_TESTNET" => &TESTNET_POLYGON_CHAIN_PARAMS,
            _ => return Err(ConfigError::UnsupportedChainName),
        };

        let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
        let db_name = env::var("DB_NAME").expect("DB_NAME must be set");
        let use_kms = get_bool_env_var("USE_KMS", false);
        let interval = get_env_var("INTERVAL", "10")?; // sec
        let min_managed_amount = get_env_var("MIN_MANAGED_AMOUNT", "1000.0")?;
        let max_managed_amount = get_env_var("MAX_MANAGED_AMOUNT", "10000.0")?;
        let trading_amount = get_env_var("TRADING_AMOUNT", "100.0")?;
        let allowance_factor = get_env_var("ALLOWANCE_FACTOR", "10000000000.0")?;
        let deadline_secs = get_env_var("DEADLINE_SECS", "60")?;
        let log_limit = get_env_var("LOG_LIMIT", "10000")?;
        let num_swaps = get_env_var("NUM_SWAPS", "3")?;
        let short_trade_period_minutes: usize = get_env_var("SHORT_TRADE_PEREIOD_MINUTES", "60")?;
        let medium_trade_period_minutes: usize =
            get_env_var("MEDIUM_TRADE_PEREIOD_MINUTES", "720")?;
        let long_trade_period_minutes: usize = get_env_var("LONG_TRACE_PEREIOD_MINUTES", "1440")?;
        let max_price_size_hours: u32 = get_env_var("MAX_PRICE_SIZE_HOURS", "36")?;

        let position_creation_inteval_hours_str =
            env::var("POSITION_CREATION_INVERVAL_HOURS").unwrap_or_default();
        let position_creation_inteval_hours: Option<u64> =
            position_creation_inteval_hours_str.parse().ok();
        let position_creation_inteval_seconds = match position_creation_inteval_hours {
            Some(value) => Some(value * 60 * 60),
            None => None,
        };

        let risk_reward = get_env_var("RISK_REWARD", "1.5")?;
        let max_error_count = get_env_var("MAX_ERROR_COUNT", "3")?;
        let reward_multiplier = get_env_var("REWARD_MULTIPLIER", "2.0")?;
        let penalty_multiplier = get_env_var("PENALTY_MULTIPLIER", "0.5")?;
        let relative_spread = get_env_var("RELATIVE_SPREAD", "0.007")?;
        let save_prices = get_bool_env_var("SAVE_PRICES", false);

        let treasury_str = env::var("TREASURY").unwrap_or_default();
        let treasury: Option<Address> = Some(Address::from_str(&treasury_str).unwrap_or_default());

        let short_trade_period = short_trade_period_minutes * 60 / (interval as usize);
        let medium_trade_period = medium_trade_period_minutes * 60 / (interval as usize);
        let long_trade_period = long_trade_period_minutes * 60 / (interval as usize);
        let max_price_size = max_price_size_hours * 60 * 60 / (interval as u32);

        let env_config = EnvConfig {
            chain_params,
            rpc_node_index,
            mongodb_uri,
            db_name,
            use_kms,
            interval,
            min_managed_amount,
            max_managed_amount,
            trading_amount,
            allowance_factor,
            deadline_secs,
            log_limit,
            dry_run,
            num_swaps,
            short_trade_period,
            medium_trade_period,
            long_trade_period,
            max_price_size,
            position_creation_inteval_seconds,
            risk_reward,
            max_error_count,
            reward_multiplier,
            penalty_multiplier,
            relative_spread,
            save_prices,
            treasury,
        };

        env_configs.push(env_config);
    }

    Ok(env_configs)
}
