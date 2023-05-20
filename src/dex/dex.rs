// dex.rs

use crate::token::Token;
use async_trait::async_trait;
use ethers::prelude::LocalWallet;
use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, U256},
};
use std::{error::Error, sync::Arc};

#[derive(Debug, Clone)]
pub struct BaseDex {
    pub provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    pub router_address: Address,
    router_contract:
        Option<Contract<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>>,
}

impl BaseDex {
    pub fn new(
        provider: Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        router_address: Address,
    ) -> Self {
        Self {
            provider: provider,
            router_address: router_address,
            router_contract: None,
        }
    }

    pub async fn create_router_contract(
        &mut self,
        abi_json: &[u8],
    ) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        if self.router_contract.is_none() {
            let router_abi = Abi::load(abi_json)?;
            let router_contract =
                Contract::new(self.router_address, router_abi, self.provider.clone());
            self.router_contract = Some(router_contract);
        }
        Ok(())
    }

    pub fn provider(
        &self,
    ) -> Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>> {
        self.provider.clone()
    }

    pub fn router_address(&self) -> Address {
        self.router_address
    }

    pub fn router_contract(
        &self,
    ) -> Result<
        &Contract<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        Box<dyn Error + Send + Sync + 'static>,
    > {
        match &self.router_contract {
            Some(contract) => Ok(contract),
            None => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Router contract not created",
            ))),
        }
    }
}

pub struct TokenPair<'a> {
    input_token: &'a dyn Token,
    output_token: &'a dyn Token,
}

impl<'a> TokenPair<'a> {
    pub fn new(input_token: &'a dyn Token, output_token: &'a dyn Token) -> Self {
        TokenPair {
            input_token,
            output_token,
        }
    }
}

#[async_trait]
pub trait Dex: Send + Sync {
    async fn get_token_price(
        &self,
        token_pair: &TokenPair<'_>,
        amount: f64,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let input_address = token_pair.input_token.address();
        let output_address = token_pair.output_token.address();

        let input_decimals = token_pair.input_token.decimals().unwrap();
        let output_decimals = token_pair.output_token.decimals().unwrap();

        let amount_in = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(input_decimals as i32)
        ))?;

        let router_contract = self.router_contract().unwrap();
        let amounts_out: Vec<U256> = router_contract
            .method::<_, Vec<U256>>(
                "getAmountsOut",
                (amount_in, vec![input_address, output_address]),
            )?
            .call()
            .await?;
        let output_amount: U256 = amounts_out[1];

        let price_f64 = output_amount.as_u128() as f64 / amount_in.as_u128() as f64
            * 10f64.powi(input_decimals as i32 - output_decimals as i32);
        log::trace!(
            "Dex: {}, Input Amount: {}, Output Amount: {}, Price: {}",
            self.name(),
            amount_in,
            output_amount,
            price_f64
        );

        Ok(price_f64)
    }

    async fn swap_token(
        &self,
        token_pair: &TokenPair<'_>,
        amount: f64,
        wallet_and_provider: Arc<
            NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
        >,
        address: Address,
        deadline_secs: u64,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let input_address = token_pair.input_token.address();
        let output_address = token_pair.output_token.address();

        let input_decimals = token_pair.input_token.decimals().unwrap();
        let amount_in = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(input_decimals as i32)
        ))?;

        let router_contract = self.router_contract().unwrap();

        let deadline = U256::from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
                + deadline_secs,
        );

        let connected_contract = router_contract.connect(wallet_and_provider.clone());

        let method_call = connected_contract.method::<_, bool>(
            "swapExactTokensForTokens",
            (
                amount_in,
                U256::zero(),
                vec![input_address, output_address],
                address,
                deadline,
            ),
        )?;

        let swap_transaction = method_call.send().await?;

        let transaction_receipt = swap_transaction.confirmations(1).await?; // wait for 1 confirmation
        if transaction_receipt.unwrap().status != Some(1.into()) {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Token swap transaction failed",
            )));
        }

        let output_amount = self.get_token_price(token_pair, amount).await?;

        Ok(output_amount)
    }

    async fn initialize(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn clone_box(&self) -> Box<dyn Dex + Send + Sync>;
    fn name(&self) -> &str;
    fn provider(
        &self,
    ) -> Arc<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>;
    fn router_address(&self) -> Address;
    fn router_contract(
        &self,
    ) -> Result<
        &Contract<NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>>,
        Box<dyn Error + Send + Sync + 'static>,
    >;
}

impl Clone for Box<dyn Dex> {
    fn clone(&self) -> Box<dyn Dex> {
        self.clone_box()
    }
}
