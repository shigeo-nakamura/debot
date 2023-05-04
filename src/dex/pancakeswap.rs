use std::{error::Error, str::FromStr};

use async_trait::async_trait;
use ethers::{abi::Abi, prelude::*, types::Address};

use crate::addresses::PANCAKE_SWAP_ROUTER;

use super::Dex;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PancakeSwap {
    pub router_address: Address,
    pub provider: Arc<Provider<Http>>,
}

static PANCAKESWAP_ROUTER_ABI_JSON: &'static [u8] =
    include_bytes!("../../resources/PancakeSwapRouterABI.json");

impl PancakeSwap {
    pub fn new(provider: Provider<Http>) -> Self {
        let router_address = Address::from_str(PANCAKE_SWAP_ROUTER).unwrap();
        Self {
            router_address,
            provider: Arc::new(provider),
        }
    }
}

#[async_trait]
impl Dex for PancakeSwap {
    fn clone_box(&self) -> Box<dyn Dex + Send + Sync> {
        Box::new(self.clone())
    }

    fn get_name(&self) -> &str {
        "PancakeSwap"
    }

    async fn swap_tokens(
        &self,
        _input_token: Address,
        _output_token: Address,
        _amount: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // implementation for PancakeSwap
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
        PANCAKESWAP_ROUTER_ABI_JSON
    }

    fn get_provider(&self) -> Arc<Provider<Http>> {
        self.provider.clone()
    }
}
