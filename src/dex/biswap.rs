use std::{error::Error, str::FromStr};

use async_trait::async_trait;
use ethers::{abi::Abi, prelude::*, types::Address};

use crate::addresses::BI_SWAP_ROUTER;

use super::Dex;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BiSwap {
    pub router_address: Address,
    pub provider: Arc<Provider<Http>>,
}

static BI_SWAP_ROUTER_ABI_JSON: &'static [u8] =
    include_bytes!("../../resources/BiSwapRouterABI.json");

impl BiSwap {
    pub fn new(provider: Provider<Http>) -> Self {
        let router_address = Address::from_str(BI_SWAP_ROUTER).unwrap();
        Self {
            router_address,
            provider: Arc::new(provider),
        }
    }
}

#[async_trait]
impl Dex for BiSwap {
    fn clone_box(&self) -> Box<dyn Dex + Send + Sync> {
        Box::new(self.clone())
    }

    fn get_name(&self) -> &str {
        "BiSwap"
    }

    async fn swap_tokens(
        &self,
        input_token: Address,
        output_token: Address,
        amount: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // implementation for BiSwap
        // ...
        Ok(())
    }

    fn router_contract(
        &self,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn Error + Send + Sync>> {
        let router_abi = Abi::load(abi_json)?;
        let router_contract = Contract::new(
            self.router_address,
            router_abi,
            self.provider.clone().into(),
        );
        Ok(router_contract)
    }

    fn router_abi_json(&self) -> &'static [u8] {
        BI_SWAP_ROUTER_ABI_JSON
    }

    fn get_provider(&self) -> Arc<Provider<Http>> {
        self.provider.clone()
    }
}
