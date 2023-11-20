use std::env;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

use debot_utils::decrypt_data_with_kms;

#[derive(Debug)]
pub struct EnvConfig {
    pub mongodb_uri: String,
    pub db_name: String,
    pub log_limit: u32,
    pub dry_run: bool,
    pub max_price_size: u32,
    pub risk_reward: f64,
    pub max_error_duration: u64,
    pub save_prices: bool,
    pub load_prices: bool,
    pub dex_router_api_key: String,
    pub dex_router_url: String,
    pub interval_msec: u64,
    pub cross_effective_duration_secs: i64,
}

#[derive(Debug)]
pub enum ConfigError {
    ParseIntError(ParseIntError),
    ParseFloatError(ParseFloatError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
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

pub async fn get_config_from_env() -> Result<EnvConfig, ConfigError> {
    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let db_name = env::var("DB_NAME").expect("DB_NAME must be set");
    let log_limit = get_env_var("LOG_LIMIT", "10000")?;
    let dry_run = get_bool_env_var("DRY_RUN", false);
    let max_price_size_hours: u32 = get_env_var("MAX_PRICE_SIZE_HOURS", "4")?;

    let risk_reward = get_env_var("RISK_REWARD", "1.5")?;
    let max_error_duration = get_env_var("MAX_ERROR_DURATION", "60")?;
    let save_prices = get_bool_env_var("SAVE_PRICES", false);
    let load_prices = get_bool_env_var("LOAD_PRICES", false);

    let max_price_size = max_price_size_hours * 60 * 60;

    let encrypted_data_key = env::var("ENCRYPTED_DATA_KEY")
        .expect("ENCRYPTED_DATA_KEY must be set")
        .replace(" ", ""); // Remove whitespace characters

    let encrypted_dex_router_api_key = env::var("ENCRYPTED_DEX_ROUTER_API_KEY")
        .expect("ENCRYPTED_DEX_ROUTER_API_KEY must be set")
        .replace(" ", ""); // Remove whitespace characters

    let dex_router_api_key =
        decrypt_data_with_kms(encrypted_data_key, encrypted_dex_router_api_key)
            .await
            .unwrap();
    let dex_router_api_key = String::from_utf8(dex_router_api_key).unwrap();

    let dex_router_url = env::var("DEX_ROUTER_URL").expect("DEX_ROUTER_URL must be set");

    let interval_msec = get_env_var("INTERVAL_MSEC", "1000")?;

    let cross_effective_duration_secs = get_env_var("CROSS_EFFECTIVE_DURATION_SECS", "10")?;

    let env_config = EnvConfig {
        mongodb_uri,
        db_name,
        log_limit,
        dry_run,
        max_price_size,
        risk_reward,
        max_error_duration,
        save_prices,
        load_prices,
        dex_router_api_key,
        dex_router_url,
        interval_msec,
        cross_effective_duration_secs,
    };

    Ok(env_config)
}
