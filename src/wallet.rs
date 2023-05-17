// wallet.rs

use ethers::signers::{LocalWallet, Signer};
use ethers::utils::hex;
use ethers_middleware::core::k256::elliptic_curve::SecretKey;
use rusoto_core::Region;
use rusoto_kms::KmsClient;
use std::{error::Error, sync::Arc};

// Define your AWS KMS Signer
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

pub fn create_local_wallet() -> Result<Arc<LocalWallet>, Box<dyn Error>> {
    let private_key_bytes =
        hex::decode("dd84b3084618a0ff534b482c5e3665b53805ce97c7ed1a46e39b671b3b897047")?;
    let secret_key = SecretKey::from_slice(&private_key_bytes)?;

    let wallet = LocalWallet::from(secret_key);
    Ok(Arc::new(wallet))
}

pub fn create_kms_wallet(key_id: String) -> Result<Arc<AwsKmsSigner>, Box<dyn Error>> {
    let client = KmsClient::new(Region::UsEast1); // choose your region
    let signer = AwsKmsSigner {
        client: Some(client),
        key_id,
    };
    Ok(Arc::new(signer))
}
