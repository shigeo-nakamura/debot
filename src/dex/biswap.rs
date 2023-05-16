// BiSwap.rs
use super::dex::BaseDex;
use super::Dex;
use async_trait::async_trait;
use ethers::{prelude::*, types::Address};
use std::error::Error;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BiSwap {
    base_dex: BaseDex,
}

static BISWAP_ROUTER_ABI_JSON: &'static [u8] =
    include_bytes!("../../resources/BiSwapRouterABI.json");

impl BiSwap {
    pub fn new(provider: Arc<Provider<Http>>, router_address: Address) -> Self {
        Self {
            base_dex: BaseDex {
                router_address,
                provider,
            },
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

    fn router_contract(
        &self,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn Error + Send + Sync>> {
        self.base_dex.router_contract(abi_json)
    }

    fn router_abi_json(&self) -> &'static [u8] {
        BISWAP_ROUTER_ABI_JSON
    }

    fn get_provider(&self) -> Arc<Provider<Http>> {
        self.base_dex.get_provider()
    }

    fn get_router_address(&self) -> Address {
        self.base_dex.get_router_address()
    }
}
