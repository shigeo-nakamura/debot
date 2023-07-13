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
    pub dex_index: usize,
    pub mongodb_uri: String,
    pub db_name: String,
    pub use_kms: bool,
    pub interval: u64,
    pub leverage: f64,
    pub min_managed_amount: f64,
    pub max_managed_amount: f64,
    pub min_trading_amount: f64,
    pub allowance_factor: f64,
    pub deadline_secs: u64,
    pub log_limit: u32,
    pub skip_write: bool,
    pub num_swaps: usize,
    pub short_trade_period: usize,
    pub medium_trade_period: usize,
    pub long_trade_period: usize,
    pub max_price_size: u32,
    pub position_creation_inteval_period: u64,
    pub flash_crash_threshold: f64,
    pub max_error_count: u32,
    pub reward_multiplier: f64,
    pub penalty_multiplier: f64,
    pub slippage: f64,
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
        let mut dex_index = 0;

        let chain_name = chain_name.trim(); // To handle spaces after the comma
        let chain_params: &'static ChainParams = match chain_name {
            "BSC" => {
                rpc_node_index = bsc_index;
                dex_index = bsc_index;
                bsc_index += 1;
                &BSC_CHAIN_PARAMS
            }
            "POLYGON" => {
                rpc_node_index = polygon_index;
                dex_index = polygon_index;
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
        let leverage = get_env_var("LEVERAGE", "0.2")?;
        let min_managed_amount = get_env_var("min_managed_amount", "500.0")?;
        let max_managed_amount = get_env_var("max_managed_amount", "4500.0")?;
        let min_trading_amount = get_env_var("min_trading_amount", "10.0")?;
        let allowance_factor = get_env_var("ALLOWANCE_FACTOR", "10000000000.0")?;
        let deadline_secs = get_env_var("DEADLINE_SECS", "60")?;
        let log_limit = get_env_var("LOG_LIMIT", "10000")?;
        let skip_write = get_bool_env_var("SKIP_WRITE", true);
        let num_swaps = get_env_var("NUM_SWAPS", "3")?;
        let short_trade_period = get_env_var("SHORT_TRADE_PEREIOD", "60")?; // 600sec = 10min
        let medium_trade_period = get_env_var("MEDIUM_TRADE_PEREIOD", "420")?; // 4200sec = 70min
        let long_trade_period = get_env_var("LONG_TRACE_PEREIOD", "1260")?; // 12600sec = 210min
        let max_price_size = get_env_var("MAX_PRICE_SIZE", "8640")?; // 86400sec = 1day
        let position_creation_inteval_period =
            get_env_var("POSITION_CREATION_INVERVAL_PERIOD", "360")?; // 1h
        let flash_crash_threshold = get_env_var("FLASH_CRASH_THRESHOLD", "0.95")?;
        let max_error_count = get_env_var("MAX_ERROR_COUNT", "3")?;
        let reward_multiplier = get_env_var("REWARD_MULTIPLIER", "1.5")?;
        let penalty_multiplier = get_env_var("PENALTY_MULTIPLIER", "0.9")?;
        let slippage = get_env_var("SLIPPAGE", "0.03")?;

        let treasury_str = env::var("TREASURY").unwrap_or_default();
        let treasury: Option<Address> = Some(Address::from_str(&treasury_str).unwrap_or_default());

        let env_config = EnvConfig {
            chain_params,
            rpc_node_index,
            dex_index,
            mongodb_uri,
            db_name,
            use_kms,
            interval,
            leverage,
            min_managed_amount,
            max_managed_amount,
            min_trading_amount,
            allowance_factor,
            deadline_secs,
            log_limit,
            skip_write,
            num_swaps,
            short_trade_period,
            medium_trade_period,
            long_trade_period,
            max_price_size,
            position_creation_inteval_period,
            flash_crash_threshold,
            max_error_count,
            reward_multiplier,
            penalty_multiplier,
            slippage,
            treasury,
        };

        env_configs.push(env_config);
    }

    Ok(env_configs)
}
