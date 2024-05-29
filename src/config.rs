use debot_market_analyzer::TradingStrategy;
use debot_market_analyzer::TrendType;
use debot_utils::decrypt_data_with_kms;
use rust_decimal::Decimal;
use rust_decimal::Error as DecimalParseError;
use std::env;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};

#[derive(Debug)]
pub struct RabbitxConfig {
    pub profile_id: String,
    pub api_key: String,
    pub public_jwt: String,
    pub refresh_token: String,
    pub secret: String,
    pub private_jwt: String,
}

#[derive(Debug)]
pub struct HyperliquidConfig {
    pub agent_private_key: String,
    pub evm_wallet_address: String,
    pub vault_address: Option<String>,
}

#[derive(Debug)]
pub struct EnvConfig {
    pub mongodb_uri: String,
    pub db_name: String,
    pub log_limit: u32,
    pub dry_run: bool,
    pub max_price_size: u32,
    pub max_error_duration: u64,
    pub save_prices: bool,
    pub load_prices: bool,
    pub interval_msec: u64,
    pub liquidate_when_exit: bool,
    pub max_dd_ratio: Decimal,
    pub order_effective_duration_secs: i64,
    pub max_open_duration_secs: i64,
    pub use_market_order: bool,
    pub risk_reward: Decimal,
    pub rest_endpoint: String,
    pub web_socket_endpoint: String,
    pub leverage: u32,
    pub strategy: Option<TradingStrategy>,
    pub load_score: bool,
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

pub fn get_config_from_env() -> Result<EnvConfig, ConfigError> {
    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let db_name = env::var("DB_NAME").expect("DB_NAME must be set");
    let log_limit = get_env_var("LOG_LIMIT", "100000")?;
    let dry_run = get_bool_env_var("DRY_RUN", true);
    let interval_msec = get_env_var("INTERVAL_MSEC", "1000")?;

    let max_price_size_hours: u32 = get_env_var("MAX_PRICE_SIZE_HOURS", "1")?;
    let max_price_size: u32 = max_price_size_hours * 60 * 60 * 1000 / interval_msec as u32;

    let max_error_duration = get_env_var("MAX_ERROR_DURATION", "10")?;
    let save_prices = get_bool_env_var("SAVE_PRICES", false);
    let load_prices = get_bool_env_var("LOAD_PRICES", false);

    let liquidate_when_exit = get_bool_env_var("LIQUIDATE_WHEN_EXIT", false);
    let max_dd_ratio = get_env_var("MAX_DD_RATIO", "0.1").map_err(ConfigError::from)?;
    let order_effective_duration_secs = get_env_var("ORDER_EFFECTIVE_DURATION_SECS", "300")?;
    let max_open_duration_secs = get_env_var("MAX_OPEN_DURATION_SECS", "3600")?;
    let use_market_order = get_bool_env_var("USE_MARKET_ORDER", false);

    let risk_reward = get_env_var("RISK_REWARD", "1.0")?;

    let rest_endpoint = env::var("REST_ENDPOINT").expect("REST_ENDPOINT must be set");
    let web_socket_endpoint =
        env::var("WEB_SOCKET_ENDPOINT").expect("WEB_SOCKET_ENDPOINT must be set");

    let leverage = get_env_var("LEVERAGE", "5")?;

    let strategy = match env::var("TRADING_STRATEGY").unwrap_or_default().as_str() {
        "randomwalk" => Some(TradingStrategy::RandomWalk(TrendType::Unknown)),
        "machinelearning" => Some(TradingStrategy::MachineLearning(TrendType::Unknown)),
        "marketmake" => Some(TradingStrategy::MarketMake),
        &_ => None,
    };

    let load_score = get_bool_env_var("LOAD_SCORE", false);

    let env_config = EnvConfig {
        mongodb_uri,
        db_name,
        log_limit,
        dry_run,
        max_price_size,
        max_error_duration,
        save_prices,
        load_prices,
        interval_msec,
        liquidate_when_exit,
        max_dd_ratio,
        order_effective_duration_secs,
        max_open_duration_secs,
        use_market_order,
        risk_reward,
        rest_endpoint,
        web_socket_endpoint,
        leverage,
        strategy,
        load_score,
    };

    Ok(env_config)
}

pub async fn get_rabbitx_config_from_env() -> Result<RabbitxConfig, ConfigError> {
    let profile_id = env::var("RABBITX_PROFILE_ID").expect("RABBITX_PROFILE_ID must be set");
    let api_key = env::var("RABBITX_API_KEY").expect("RABBITX_API_KEY must be set");
    let public_jwt = env::var("RABBITX_PUBLIC_JWT").expect("RABBITX_PUBLIC_JWT must be set");
    let refresh_token =
        env::var("RABBITX_REFRESH_TOKEN").expect("RABBITX_REFRESH_TOKEN must be set");
    let secret = env::var("RABBITX_SECRET").expect("RABBITX_SECRET must be set");
    let private_jwt = env::var("RABBITX_PRIVATE_JWT").expect("RABBITX_PRIVATE_JWT must be set");

    let encrypted_data_key = env::var("ENCRYPTED_DATA_KEY")
        .expect("ENCRYPTED_DATA_KEY must be set")
        .replace(" ", ""); // Remove whitespace characters

    let secret_vec = decrypt_data_with_kms(&encrypted_data_key, secret, true)
        .await
        .map_err(|_| ConfigError::OtherError("decrypt secret".to_owned()))?;
    let secret = String::from_utf8(secret_vec).unwrap();

    let private_jwt_vec = decrypt_data_with_kms(&encrypted_data_key, private_jwt, false)
        .await
        .map_err(|_| ConfigError::OtherError("decrypt private_jwt".to_owned()))?;
    let private_jwt = String::from_utf8(private_jwt_vec).unwrap();

    Ok(RabbitxConfig {
        profile_id,
        api_key,
        public_jwt,
        refresh_token,
        secret,
        private_jwt,
    })
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
