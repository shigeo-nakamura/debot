// Polygon_token.rs

use super::token::{BaseToken, BlockChain, Token};
use ethers::types::Address;
use std::error::Error;

#[derive(Clone)]
pub struct PolygonToken {
    base_token: BaseToken,
}

#[async_trait::async_trait]
impl Token for PolygonToken {
    fn new(
        block_chain: BlockChain,
        rpc_node_url: String,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
    ) -> Self {
        Self {
            base_token: BaseToken::new(
                block_chain,
                rpc_node_url,
                address,
                symbol_name,
                decimals,
                fee_rate,
            ),
        }
    }

    fn clone_boxed(&self) -> Box<dyn Token> {
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

    fn rpc_node_url(&self) -> &str {
        self.base_token.rpc_node_url()
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
}
