// token.rs

use ethers::types::U256;
use ethers::{
    abi::Abi, contract::Contract, middleware::SignerMiddleware, providers::Http,
    providers::Provider, signers::LocalWallet, types::Address,
};

use std::error::Error;
use std::sync::Arc;
static ERC20_TOKEN_ABI_JSON: &'static [u8] = include_bytes!("../../resources/ERC20TokenABI.json");

#[derive(Clone)]
pub enum BlockChain {
    BscChain { chain_id: u64 },
    PolygonChain { chain_id: u64 },
}

#[derive(Clone)]
pub struct BaseToken {
    block_chain: BlockChain,
    provider: Arc<Provider<Http>>,
    address: Address,
    symbol_name: String,
    decimals: Option<u8>,
    fee_rate: f64,
    wallet: Arc<LocalWallet>,
}

impl BaseToken {
    pub fn new(
        block_chain: BlockChain,
        provider: Arc<Provider<Http>>,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
        wallet: Arc<LocalWallet>,
    ) -> Self {
        Self {
            block_chain,
            provider,
            address,
            symbol_name,
            decimals,
            fee_rate,
            wallet,
        }
    }

    pub fn block_chain_id(&self) -> u64 {
        match &self.block_chain {
            BlockChain::BscChain { chain_id } => *chain_id,
            BlockChain::PolygonChain { chain_id } => *chain_id,
        }
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
        let decimals: u8 = self
            .token_contract()?
            .method("decimals", ())?
            .call()
            .await?;
        self.decimals = Some(decimals);
        Ok(())
    }

    pub fn token_contract(
        &self,
    ) -> Result<
        Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>,
        Box<dyn Error + Send + Sync>,
    > {
        let client = SignerMiddleware::new(self.provider.clone(), (*self.wallet).clone());
        let token_contract = Contract::new(
            self.address,
            Abi::load(ERC20_TOKEN_ABI_JSON)?,
            client.into(),
        );
        Ok(token_contract)
    }
    pub async fn approve(
        &self,
        spender: Address,
        amount: U256,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let contract = self.token_contract()?;
        let call = contract.method::<_, ()>("approve", (spender, amount))?;
        call.send().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Token: Send + Sync {
    fn new(
        block_chain: BlockChain,
        provider: Arc<Provider<Http>>,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
        wallet: Arc<LocalWallet>,
    ) -> Self
    where
        Self: Sized;
    fn clone_boxed(&self) -> Box<dyn Token>;
    async fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn block_chain(&self) -> BlockChain;
    fn block_chain_id(&self) -> u64;
    fn address(&self) -> Address;
    fn symbol_name(&self) -> &str;
    async fn decimals(&self) -> Result<u8, Box<dyn Error + Send + Sync>>;
    fn fee_rate(&self) -> f64;
    async fn approve(
        &self,
        spender: Address,
        amount: U256,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

impl Clone for Box<dyn Token> {
    fn clone(&self) -> Box<dyn Token> {
        self.clone_boxed()
    }
}
