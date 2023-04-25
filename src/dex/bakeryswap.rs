use std::error::Error;
use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use ethers::{abi::Abi, prelude::*, types::Address};

use crate::addresses::BAKERY_SWAP_ROUTER;

use super::Dex;

#[derive(Debug, Clone)]
pub struct BakerySwap {
    pub router_address: Address,
    pub provider: Arc<Provider<Http>>,
}

static BAKERY_SWAP_ROUTER_ABI_JSON: &'static [u8] =
    include_bytes!("../../resources/BakerySwapRouterABI.json");

impl BakerySwap {
    pub fn new(provider: Provider<Http>) -> Self {
        let router_address = Address::from_str(BAKERY_SWAP_ROUTER).unwrap();
        Self {
            router_address,
            provider: Arc::new(provider),
        }
    }
}

#[async_trait]
impl Dex for BakerySwap {
    fn clone_box(&self) -> Box<dyn Dex + Send + Sync> {
        Box::new(self.clone())
    }

    fn get_name(&self) -> &str {
        "BakerySwap"
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
        BAKERY_SWAP_ROUTER_ABI_JSON
    }

    fn get_provider(&self) -> Arc<Provider<Http>> {
        self.provider.clone()
    }
}
