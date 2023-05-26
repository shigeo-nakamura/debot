// bsc_token.rs

use super::token::{BaseToken, BlockChain, Token};
use ethers::{
    signers::LocalWallet,
    types::{Address, U256},
};
use ethers_middleware::{
    providers::{Http, Provider},
    NonceManagerMiddleware, SignerMiddleware,
};
use std::{error::Error, sync::Arc};

#[derive(Clone)]
pub struct BscToken {
    base_token: BaseToken,
}

#[async_trait::async_trait]
impl Token for BscToken {
    fn new(
        block_chain: BlockChain,
        provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
    ) -> Self {
        Self {
            base_token: BaseToken::new(block_chain, provider, address, symbol_name, decimals),
        }
    }

    fn clone_box(&self) -> Box<dyn Token> {
        Box::new(self.clone())
    }

    fn block_chain(&self) -> BlockChain {
        BlockChain::BscChain {
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

    fn decimals(&self) -> Option<u8> {
        self.base_token.decimals()
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
