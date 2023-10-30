// wallet.rs

use bigdecimal::num_bigint::BigUint;
use bigdecimal::BigDecimal;
use bigdecimal::ToPrimitive;
use debot_utils::decrypt_data_with_kms;
use ethers::signers::{LocalWallet, Signer};
use ethers::utils::hex::{self, encode};
use ethers_middleware::core::k256::elliptic_curve::SecretKey;
use ethers_middleware::providers::{Http, Provider};
use ethers_middleware::MiddlewareBuilder;
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use std::env;
use std::str::FromStr;
use std::{error::Error, sync::Arc};
use web3::types::Address;
use web3::Web3;

use crate::blockchain_factory::ChainParams;

const TOKEN_DECIMALS: usize = 18;

pub async fn create_wallet(
    chain_params: &ChainParams,
    rpc_node_index: usize,
    use_kms: bool,
) -> Result<
    (
        LocalWallet,
        Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    ),
    Box<dyn Error>,
> {
    let private_key_hex_string = if use_kms {
        let decrypted_data_hex = decrypt_data_with_kms().await?;
        encode(&decrypted_data_hex)
    } else {
        env::var("PRIVATE_KEY").expect("No private key given")
    };

    let private_key_bytes = hex::decode(&private_key_hex_string)?;
    let secret_key = SecretKey::from_slice(&private_key_bytes)?;

    let provider = Provider::<Http>::try_from(chain_params.rpc_node_urls[rpc_node_index])?;

    let wallet = LocalWallet::from(secret_key).with_chain_id(chain_params.chain_id);
    let provider = provider.with_signer(wallet.clone());

    let nonce_manager = NonceManagerMiddleware::new(provider, wallet.address());

    Ok((wallet, Arc::new(nonce_manager)))
}

pub async fn get_balance_of_native_token(
    chain_params: &ChainParams,
    owner: Address,
) -> Result<f64, Box<dyn std::error::Error>> {
    get_balance_in_wallet(chain_params.rpc_node_urls[0], owner).await
}

pub async fn get_balance_in_wallet(
    rpc_url: &str,
    owner: Address,
) -> Result<f64, Box<dyn std::error::Error>> {
    let http = web3::transports::Http::new(rpc_url)?;
    let web3 = Web3::new(http);

    let result = web3.eth().balance(owner, None).await?;

    // Convert Wei to native token
    let balance = BigDecimal::from_str(&result.to_string())?;

    // Calculate power of 10 for TOKEN_DECIMALS using num-bigint and num-traits
    let pow_val = BigUint::from(10u32).pow((TOKEN_DECIMALS as usize).try_into().unwrap());
    let wei_in_one_token = BigDecimal::from_str(&pow_val.to_str_radix(10))?;
    let balance = balance / wei_in_one_token;

    // Convert BigDecimal to f64
    let balance = balance
        .to_f64()
        .ok_or("Failed to convert BigDecimal to f64")?;

    Ok(balance)
}
