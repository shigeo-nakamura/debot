// wallet.rs

use ethers::signers::{LocalWallet, Signer};
use ethers_middleware::core::k256::elliptic_curve::SecretKey;
use ethers_middleware::providers::{Http, Provider};
use ethers_middleware::MiddlewareBuilder;
use ethers_middleware::{NonceManagerMiddleware, SignerMiddleware};
use hex_literal::hex;
use rusoto_core::Region;
use rusoto_kms::KmsClient;
use std::{error::Error, sync::Arc};

use crate::token_manager::ChainParams;

use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};

lazy_static! {
    static ref INDEX: AtomicUsize = AtomicUsize::new(0);
}

// Define your AWS KMS Signer
#[derive(Clone)]
pub struct AwsKmsSigner {
    client: Option<KmsClient>,
    key_id: String,
}

impl std::fmt::Debug for AwsKmsSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsKmsSigner")
            .field("client", &self.client.is_some())
            .field("key_id", &self.key_id)
            .finish()
    }
}

impl Signer for AwsKmsSigner {
    type Error = std::io::Error;

    fn sign_message<'life0, 'async_trait, S>(
        &'life0 self,
        message: S,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<ethers::types::Signature, Self::Error>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        S: 'async_trait + Send + Sync + AsRef<[u8]>,
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    fn sign_transaction<'life0, 'life1, 'async_trait>(
        &'life0 self,
        message: &'life1 ethers::types::transaction::eip2718::TypedTransaction,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<ethers::types::Signature, Self::Error>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    fn sign_typed_data<'life0, 'life1, 'async_trait, T>(
        &'life0 self,
        payload: &'life1 T,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<ethers::types::Signature, Self::Error>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: 'async_trait + ethers::types::transaction::eip712::Eip712 + Send + Sync,
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    fn address(&self) -> ethers::types::Address {
        todo!()
    }

    fn chain_id(&self) -> u64 {
        todo!()
    }

    fn with_chain_id<T: Into<u64>>(self, chain_id: T) -> Self {
        todo!()
    }
}

pub fn create_local_wallet(
    chain_params: &ChainParams,
) -> Result<
    (
        LocalWallet,
        Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    ),
    Box<dyn Error>,
> {
    let private_key_bytes =
        hex!("dd84b3084618a0ff534b482c5e3665b53805ce97c7ed1a46e39b671b3b897047");
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

pub fn create_kms_wallet(
    chain_params: &ChainParams,
    key_id: String,
) -> Result<
    Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, AwsKmsSigner>>>,
    Box<dyn Error>,
> {
    let client = KmsClient::new(Region::UsEast1); // choose your region
    let signer = AwsKmsSigner {
        client: Some(client),
        key_id,
    };

    let provider = Provider::<Http>::try_from(chain_params.rpc_node_urls[0])?;

    let wallet = signer.with_chain_id(chain_params.chain_id);
    let provider = provider.with_signer(wallet.clone());

    let nonce_manager = NonceManagerMiddleware::new(provider, wallet.address());

    Ok(Arc::new(nonce_manager))
}
