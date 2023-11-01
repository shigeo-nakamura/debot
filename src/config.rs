use std::env;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

#[derive(Debug)]
pub struct EnvConfig {
    pub mongodb_uri: String,
    pub db_name: String,
    pub interval: u64,
    pub log_limit: u32,
    pub dry_run: bool,
    pub max_price_size: u32,
    pub risk_reward: f64,
    pub max_error_count: u32,
    pub save_prices: bool,
    pub load_prices: bool,
    pub encrypted_api_key: String,
    pub dex_router_url: String,
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

pub fn get_config_from_env() -> Result<EnvConfig, ConfigError> {
    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let db_name = env::var("DB_NAME").expect("DB_NAME must be set");
    let interval = get_env_var("INTERVAL", "10")?; // sec
    let log_limit = get_env_var("LOG_LIMIT", "10000")?;
    let dry_run = get_bool_env_var("DRY_RUN", false);
    let max_price_size_hours: u32 = get_env_var("MAX_PRICE_SIZE_HOURS", "24")?;

    let risk_reward = get_env_var("RISK_REWARD", "1.5")?;
    let max_error_count = get_env_var("MAX_ERROR_COUNT", "3")?;
    let save_prices = get_bool_env_var("SAVE_PRICES", false);
    let load_prices = get_bool_env_var("LOAD_PRICES", false);

    let max_price_size = max_price_size_hours * 60 * 60 / (interval as u32);

    let encrypted_api_key = env::var("ENCRYPTED_API_KEY").expect("ENCRYPTED_API_KEY must be set");
    let dex_router_url = env::var("DEX_ROUTER_URL").expect("DEX_ROUTER_URL must be set");

    let env_config = EnvConfig {
        mongodb_uri,
        db_name,
        interval,
        log_limit,
        dry_run,
        max_price_size,
        risk_reward,
        max_error_count,
        save_prices,
        load_prices,
        encrypted_api_key,
        dex_router_url,
    };

    Ok(env_config)
}
