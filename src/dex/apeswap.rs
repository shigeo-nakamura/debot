// pancakeswap.rs

use super::dex::BaseDex;
use super::Dex;
use crate::addresses::BSC_APE_SWAP_ROUTER;
use async_trait::async_trait;
use ethers::{prelude::*, types::Address};
use std::sync::Arc;
use std::{error::Error, str::FromStr};

#[derive(Debug, Clone)]
pub struct ApeSwap {
    base_dex: BaseDex,
}

static APE_SWAP_ROUTER_ABI_JSON: &'static [u8] =
    include_bytes!("../../resources/ApeSwapRouterABI.json");

impl ApeSwap {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        let router_address = Address::from_str(BSC_APE_SWAP_ROUTER).unwrap();
        Self {
            base_dex: BaseDex {
                router_address,
                provider,
            },
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

    fn router_contract(
        &self,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn Error + Send + Sync>> {
        self.base_dex.router_contract(abi_json)
    }

    fn router_abi_json(&self) -> &'static [u8] {
        APE_SWAP_ROUTER_ABI_JSON
    }

    fn get_provider(&self) -> Arc<Provider<Http>> {
        self.base_dex.get_provider()
    }
}
