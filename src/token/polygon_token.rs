// Polygon_token.rs

use super::token::{BaseToken, BlockChain, Token};
use ethers::{
    signers::LocalWallet,
    types::{Address, U256},
};
use ethers_middleware::providers::{Http, Provider};
use std::{error::Error, sync::Arc};

#[derive(Clone)]
pub struct PolygonToken {
    base_token: BaseToken,
}

#[async_trait::async_trait]
impl Token for PolygonToken {
    fn new(
        block_chain: BlockChain,
        provider: Arc<Provider<Http>>,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
        wallet: Arc<LocalWallet>,
    ) -> Self {
        Self {
            base_token: BaseToken::new(
                block_chain,
                provider,
                address,
                symbol_name,
                decimals,
                fee_rate,
                wallet,
            ),
        }
    }

    fn clone_box(&self) -> Box<dyn Token> {
        Box::new(self.clone())
    }

    fn block_chain(&self) -> BlockChain {
        BlockChain::PolygonChain {
            chain_id: self.base_token.block_chain_id(),
        }
    }

    // Delegate the implementation of common methods to the BaseToken
    fn block_chain_id(&self) -> u64 {
        self.base_token.block_chain_id()
    }

    fn address(&self) -> Address {
        self.base_token.address()
    }

    fn symbol_name(&self) -> &str {
        self.base_token.symbol_name()
    }

    async fn decimals(&self) -> Result<u8, Box<dyn Error + Send + Sync>> {
        self.base_token.decimals().await
    }

    fn fee_rate(&self) -> f64 {
        self.base_token.fee_rate()
    }

    async fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_token.initialize().await
    }

    async fn approve(
        &self,
        spender: Address,
        amount: U256,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.base_token.approve(spender, amount).await
    }

    async fn allowance(
        &self,
        owner: Address,
        spender: Address,
    ) -> Result<U256, Box<dyn Error + Send + Sync>> {
        self.base_token.allowance(owner, spender).await
    }
}
