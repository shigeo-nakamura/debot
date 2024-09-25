use debot_market_analyzer::TradingStrategy;
use debot_market_analyzer::TrendType;
use debot_utils::decrypt_data_with_kms;
use rust_decimal::Decimal;
use rust_decimal::Error as DecimalParseError;
use std::env;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

#[derive(Debug)]
pub struct HyperliquidConfig {
    pub agent_private_key: String,
    pub evm_wallet_address: String,
    pub vault_address: Option<String>,
}

#[derive(Debug)]
pub struct EnvConfig {
    pub mongodb_uri: String,
    pub db_w_name: String,
    pub db_r_name: String,
    pub position_log_limit: Option<u32>,
    pub dry_run: bool,
    pub max_price_size: u32,
    pub max_error_duration: u64,
    pub save_prices: bool,
    pub load_prices: bool,
    pub interval_secs: i64,
    pub liquidate_when_exit: bool,
    pub max_dd_ratio: Decimal,
    pub close_order_effective_duration_secs: i64,
    pub use_market_order: bool,
    pub rest_endpoint: String,
    pub web_socket_endpoint: String,
    pub leverage: u32,
    pub strategy: Option<TradingStrategy>,
    pub only_read_price: bool,
    pub back_test: bool,
    pub path_to_models: Option<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    ParseIntError(ParseIntError),
    ParseFloatError(ParseFloatError),
    DecimalParseError(DecimalParseError),
    OtherError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::ParseIntError(e) => write!(f, "Parse int error: {}", e),
            ConfigError::ParseFloatError(e) => write!(f, "Parse float error: {}", e),
            ConfigError::DecimalParseError(e) => write!(f, "Decimal parse error: {}", e),
            ConfigError::OtherError(s) => write!(f, "Other error: {}", s),
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

impl From<rust_decimal::Error> for ConfigError {
    fn from(err: rust_decimal::Error) -> ConfigError {
        ConfigError::DecimalParseError(err)
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

fn get_optional_env_var<T: std::str::FromStr>(var: &str) -> Option<T> {
    match std::env::var(var) {
        Ok(val) => val.parse::<T>().ok(),
        Err(_) => None,
    }
}

pub fn get_config_from_env() -> Result<EnvConfig, ConfigError> {
    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let db_r_name = env::var("DB_R_NAME").expect("DB_R_NAME must be set");
    let db_w_name = env::var("DB_W_NAME").expect("DB_W_NAME must be set");
    let position_log_limit: Option<u32> = get_optional_env_var("POSITION_LOG_LIMIT");
    let dry_run = get_bool_env_var("DRY_RUN", true);
    let interval_secs = get_env_var("INTERVAL_SECS", "60")?;

    let max_price_size_hours: u32 = get_env_var("MAX_PRICE_SIZE_HOURS", "1")?;
    let max_price_size: u32 = max_price_size_hours * 60 * 60 / interval_secs as u32;

    let max_error_duration = get_env_var("MAX_ERROR_DURATION", "60")?;
    let save_prices = get_bool_env_var("SAVE_PRICES", false);
    let load_prices = get_bool_env_var("LOAD_PRICES", false);

    let liquidate_when_exit = get_bool_env_var("LIQUIDATE_WHEN_EXIT", true);
    let max_dd_ratio = get_env_var("MAX_DD_RATIO", "0.1").map_err(ConfigError::from)?;
    let close_order_effective_duration_secs =
        get_env_var("CLOSE_ORDER_EFFECTIVE_DURATION_SECS", "300")?;
    let use_market_order = get_bool_env_var("USE_MARKET_ORDER", false);

    let rest_endpoint = env::var("REST_ENDPOINT").expect("REST_ENDPOINT must be set");
    let web_socket_endpoint =
        env::var("WEB_SOCKET_ENDPOINT").expect("WEB_SOCKET_ENDPOINT must be set");

    let leverage = get_env_var("LEVERAGE", "1")?;

    let strategy = match env::var("TRADING_STRATEGY").unwrap_or_default().as_str() {
        "randomwalk" => Some(TradingStrategy::RandomWalk(TrendType::Unknown)),
        "meanreversion" => Some(TradingStrategy::MeanReversion(TrendType::Unknown)),
        &_ => None,
    };
    let only_read_price = get_bool_env_var("ONLY_READ_PRICE", false);
    let back_test = get_bool_env_var("BACK_TEST", false);

    let path_to_models = env::var("PATH_TO_MODELS").ok();

    let env_config = EnvConfig {
        mongodb_uri,
        db_r_name,
        db_w_name,
        position_log_limit,
        dry_run,
        max_price_size,
        max_error_duration,
        save_prices,
        load_prices,
        interval_secs,
        liquidate_when_exit,
        max_dd_ratio,
        close_order_effective_duration_secs,
        use_market_order,
        rest_endpoint,
        web_socket_endpoint,
        leverage,
        strategy,
        only_read_price,
        back_test,
        path_to_models,
    };

    Ok(env_config)
}

pub async fn get_hyperliquid_config_from_env() -> Result<HyperliquidConfig, ConfigError> {
    let agent_private_key = env::var("HYPERLIQUID_AGENT_PRIVATE_KEY")
        .expect("HYPERLIQUID_AGENT_PRIVATE_KEY must be set");
    let evm_wallet_address = env::var("HYPERLIQUID_EVM_WALLET_ADDRESS")
        .expect("HYPERLIQUID_EVM_WALLET_ADDRESS must be set");
    let vault_address = env::var("HYPERLIQUID_VAULT_ADDRESS").ok();

    let encrypted_data_key = env::var("ENCRYPTED_DATA_KEY")
        .expect("ENCRYPTED_DATA_KEY must be set")
        .replace(" ", ""); // Remove whitespace characters

    let agent_private_key_vec = decrypt_data_with_kms(&encrypted_data_key, agent_private_key, true)
        .await
        .map_err(|_| ConfigError::OtherError("decrypt agent_private_key".to_owned()))?;
    let agent_private_key = String::from_utf8(agent_private_key_vec).unwrap();

    Ok(HyperliquidConfig {
        agent_private_key,
        evm_wallet_address,
        vault_address,
    })
}
