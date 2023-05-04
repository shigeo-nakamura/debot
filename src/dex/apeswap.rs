use std::str::FromStr;
use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use ethers::{abi::Abi, prelude::*, types::Address};

use crate::addresses::APE_SWAP_ROUTER;

use super::Dex;

#[derive(Debug, Clone)]
pub struct ApeSwap {
    pub router_address: Address,
    pub provider: Arc<Provider<Http>>,
}

static APE_SWAP_ROUTER_ABI_JSON: &'static [u8] =
    include_bytes!("../../resources/ApeSwapRouterABI.json");

impl ApeSwap {
    pub fn new(provider: Provider<Http>) -> Self {
        let router_address = Address::from_str(APE_SWAP_ROUTER).unwrap();
        Self {
            router_address,
            provider: Arc::new(provider),
        }
    }
}

#[async_trait]
impl Dex for ApeSwap {
    fn clone_box(&self) -> Box<dyn Dex + Send + Sync> {
        Box::new(self.clone())
    }

    fn get_name(&self) -> &str {
        "ApeSwap"
    }

    async fn swap_tokens(
        &self,
        _input_token: Address,
        _output_token: Address,
        _amount: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // implementation for ApeSwap
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
        APE_SWAP_ROUTER_ABI_JSON
    }

    fn get_provider(&self) -> Arc<Provider<Http>> {
        self.provider.clone()
    }
}
