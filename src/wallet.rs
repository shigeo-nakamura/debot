// wallet.rs

use ethers::signers::{LocalWallet, Signer};
use ethers::utils::hex::{self, encode};
use ethers_middleware::core::k256::elliptic_curve::SecretKey;
use ethers_middleware::providers::{Http, Provider};
use ethers_middleware::MiddlewareBuilder;
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use std::env;
use std::{error::Error, sync::Arc};

use crate::blockchain_factory::ChainParams;
use crate::kws_decrypt::decrypt_data_with_kms;

use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};

lazy_static! {
    static ref INDEX: AtomicUsize = AtomicUsize::new(0);
}

pub async fn create_wallet(
    chain_params: &ChainParams,
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

    let index = INDEX.fetch_add(1, Ordering::SeqCst);
    let provider = Provider::<Http>::try_from(
        chain_params.rpc_node_urls[index % chain_params.rpc_node_urls.len()],
    )?;

    let wallet = LocalWallet::from(secret_key).with_chain_id(chain_params.chain_id);
    let provider = provider.with_signer(wallet.clone());

    let nonce_manager = NonceManagerMiddleware::new(provider, wallet.address());

    Ok((wallet, Arc::new(nonce_manager)))
}
