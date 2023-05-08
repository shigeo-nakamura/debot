// token.rs

use ethers::{abi::Abi, prelude::*, types::Address};
use ethers_middleware::providers::Provider;
use std::error::Error;

static ERC20_TOKEN_ABI_JSON: &'static [u8] = include_bytes!("../../resources/ERC20TokenABI.json");

#[derive(Clone)]
pub enum BlockChain {
    BscChain { chain_id: u64 },
    PolygonChain { chain_id: u64 },
}

#[derive(Clone)]
pub struct BaseToken {
    block_chain: BlockChain,
    rpc_node_url: String,
    address: Address,
    symbol_name: String,
    decimals: Option<u8>,
    fee_rate: f64,
}

impl BaseToken {
    pub fn new(
        block_chain: BlockChain,
        rpc_node_url: String,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
    ) -> Self {
        Self {
            block_chain,
            rpc_node_url,
            address,
            symbol_name,
            decimals,
            fee_rate,
        }
    }

    pub fn block_chain_id(&self) -> u64 {
        match &self.block_chain {
            BlockChain::BscChain { chain_id } => *chain_id,
            BlockChain::PolygonChain { chain_id } => *chain_id,
        }
    }

    pub fn rpc_node_url(&self) -> &str {
        &self.rpc_node_url
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    pub async fn decimals(&self) -> Result<u8, Box<dyn Error + Send + Sync>> {
        if let Some(decimals) = self.decimals {
            Ok(decimals)
        } else {
            let mut this = self.clone();
            this.initialize().await?;
            Ok(this.decimals.unwrap())
        }
    }

    pub fn fee_rate(&self) -> f64 {
        self.fee_rate
    }

    pub async fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let provider_result = Provider::<Http>::try_from(&self.rpc_node_url);
        let provider = match provider_result {
            Ok(p) => p,
            Err(e) => {
                log::error!("Error creating provider: {:?}", e);
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Provider error",
                )));
            }
        };

        let token_contract = Contract::new(
            self.address,
            Abi::load(ERC20_TOKEN_ABI_JSON)?,
            provider.into(),
        );
        let decimals: u8 = token_contract.method("decimals", ())?.call().await?;
        self.decimals = Some(decimals);
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Token: Send + Sync {
    fn new(
        block_chain: BlockChain,
        rpc_node_url: String,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
    ) -> Self
    where
        Self: Sized;
    fn clone_boxed(&self) -> Box<dyn Token>;
    async fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn block_chain(&self) -> BlockChain;
    fn block_chain_id(&self) -> u64;
    fn rpc_node_url(&self) -> &str;
    fn address(&self) -> Address;
    fn symbol_name(&self) -> &str;
    async fn decimals(&self) -> Result<u8, Box<dyn Error + Send + Sync>>;
    fn fee_rate(&self) -> f64;
}

impl Clone for Box<dyn Token> {
    fn clone(&self) -> Box<dyn Token> {
        self.clone_boxed()
    }
}
