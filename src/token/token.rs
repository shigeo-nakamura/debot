// token.rs

use ethers::types::U256;
use ethers::{
    abi::Abi, contract::Contract, middleware::SignerMiddleware, providers::Http,
    providers::Provider, signers::LocalWallet, types::Address,
};
use ethers_middleware::NonceManagerMiddleware;

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
    provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    address: Address,
    symbol_name: String,
    decimals: Option<u8>,
    fee_rate: f64,
    abi: Abi,
    token_contract:
        Option<Contract<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>>,
}

impl BaseToken {
    pub fn new(
        block_chain: BlockChain,
        provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
    ) -> Self {
        let abi = Abi::load(ERC20_TOKEN_ABI_JSON).unwrap();
        Self {
            block_chain,
            provider,
            address,
            symbol_name,
            decimals,
            fee_rate,
            abi,
            token_contract: None,
        }
    }

    pub async fn create_token_contract(
        &mut self,
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        if self.token_contract.is_none() {
            let token_contract =
                Contract::new(self.address, self.abi.clone(), self.provider.clone());
            self.token_contract = Some(token_contract);
        }
        Ok(())
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

    pub fn decimals(&self) -> Option<u8> {
        self.decimals
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

        self.create_token_contract().await?;
        Ok(())
    }

    pub fn token_contract(
        &self,
    ) -> Result<
        &Contract<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        Box<dyn Error + Send + Sync>,
    > {
        match &self.token_contract {
            Some(contract) => Ok(contract),
            None => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Token contract not created",
            ))),
        }
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

    pub async fn allowance(
        &self,
        owner: Address,
        spender: Address,
    ) -> Result<U256, Box<dyn Error + Send + Sync>> {
        let contract = self.token_contract()?;
        let allowance: U256 = contract
            .method("allowance", (owner, spender))?
            .call()
            .await?;
        Ok(allowance)
    }
}

#[async_trait::async_trait]
pub trait Token: Send + Sync {
    fn new(
        block_chain: BlockChain,
        provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        address: Address,
        symbol_name: String,
        decimals: Option<u8>,
        fee_rate: f64,
    ) -> Self
    where
        Self: Sized;
    fn clone_box(&self) -> Box<dyn Token>;
    async fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn block_chain(&self) -> BlockChain;
    fn block_chain_id(&self) -> u64;
    fn address(&self) -> Address;
    fn symbol_name(&self) -> &str;
    fn decimals(&self) -> Option<u8>;
    fn fee_rate(&self) -> f64;
    async fn approve(
        &self,
        spender: Address,
        amount: U256,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn allowance(
        &self,
        owner: Address,
        spender: Address,
    ) -> Result<U256, Box<dyn Error + Send + Sync>>;
}

impl Clone for Box<dyn Token> {
    fn clone(&self) -> Box<dyn Token> {
        self.clone_box()
    }
}
