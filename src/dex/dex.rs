// dex.rs

use crate::token::Token;
use async_trait::async_trait;
use ethers::prelude::LocalWallet;
use ethers::utils::parse_units;
use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, U256},
};
use std::{error::Error, sync::Arc};

use ethers::core::types::Log;

use anyhow::Error as AnyhowError;

fn parse_swap_log(log: &Log) -> Result<(f64, f64, f64, f64), AnyhowError> {
    // Check if this log has the correct number of topics and the data
    if log.topics.len() < 2 || log.data.is_empty() {
        return Err(AnyhowError::msg(
            "Log does not have enough topics/data for parsing swap",
        ));
    }

    // Convert topics to U256
    let amount0_in = U256::from(log.topics[0].as_fixed_bytes());
    let amount1_in = U256::from(log.topics[1].as_fixed_bytes());

    // Convert log.data to byte slice and then slice it
    let data = log.data.as_ref();
    let amount0_out = U256::from_big_endian(&data[0..32]);
    let amount1_out = U256::from_big_endian(&data[32..64]);

    // Convert U256 amounts to f64
    let amount0_in = amount0_in.low_u64() as f64;
    let amount1_in = amount1_in.low_u64() as f64;
    let amount0_out = amount0_out.low_u64() as f64;
    let amount1_out = amount1_out.low_u64() as f64;

    Ok((amount0_in, amount1_in, amount0_out, amount1_out))
}

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

#[derive(Clone)]
pub struct TokenPair {
    input_token: Arc<Box<dyn Token>>,
    output_token: Arc<Box<dyn Token>>,
}

impl TokenPair {
    pub fn new(input_token: Arc<Box<dyn Token>>, output_token: Arc<Box<dyn Token>>) -> Self {
        TokenPair {
            input_token,
            output_token,
        }
    }

    pub fn swap(self) -> Self {
        TokenPair {
            input_token: self.input_token,
            output_token: self.output_token,
        }
    }
}

#[async_trait]
pub trait Dex: Send + Sync {
    async fn get_token_price(
        &self,
        token_pair: &TokenPair,
        amount: f64,
        use_get_amounts_in: bool,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let input_address = token_pair.input_token.address();
        let output_address = token_pair.output_token.address();

        let input_decimals = token_pair.input_token.decimals().unwrap();
        let output_decimals = token_pair.output_token.decimals().unwrap();

        let router_contract = self.router_contract().unwrap();

        let mut amount_in = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(input_decimals as i32)
        ))?;

        let mut amount_out = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(output_decimals as i32)
        ))?;

        if use_get_amounts_in {
            let amounts_in: Vec<U256> = router_contract
                .method::<_, Vec<U256>>(
                    "getAmountsIn",
                    (amount_out, vec![input_address, output_address]),
                )?
                .call()
                .await?;
            amount_in = amounts_in[0];
        } else {
            let amounts_out: Vec<U256> = router_contract
                .method::<_, Vec<U256>>(
                    "getAmountsOut",
                    (amount_in, vec![input_address, output_address]),
                )?
                .call()
                .await?;
            amount_out = amounts_out[1];
        }

        let price_f64 = amount_out.as_u128() as f64 / amount_in.as_u128() as f64
            * 10f64.powi(input_decimals as i32 - output_decimals as i32);

        log::trace!(
            "{}, Amount-in: {}({}), Amount-out: {}({}), Price: {:6.6}",
            self.name(),
            amount_in,
            token_pair.input_token.symbol_name(),
            amount_out,
            token_pair.output_token.symbol_name(),
            price_f64
        );

        Ok(price_f64)
    }

    async fn swap_token(
        &self,
        token_pair: &TokenPair,
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

        let transaction_receipt = match transaction_receipt {
            Some(receipt) => receipt,
            None => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Transaction receipt is none",
                )))
            }
        };

        if transaction_receipt.status != Some(1.into()) {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Token swap transaction failed",
            )));
        }

        // Parse the Swap event logs to get the output amount
        let logs = transaction_receipt.logs;

        let mut output_amount: Option<U256> = None;

        for log in &logs {
            if log.address == output_address {
                let (_amount0_in, _amount1_in, amount0_out, amount1_out) = parse_swap_log(&log)?;
                output_amount = Some(parse_units(&(amount0_out + amount1_out), 18)?.into());
                break;
            }
        }

        let output_amount = output_amount.ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Output amount not found in transaction logs",
            ))
        })?;

        let output_decimals = token_pair.output_token.decimals().unwrap();
        let output_amount_in_token =
            output_amount.low_u64() as f64 / 10f64.powi(output_decimals as i32);

        Ok(output_amount_in_token)
    }

    async fn has_token_pair(&self, input_token: &dyn Token, output_token: &dyn Token) -> bool {
        let input_address = input_token.address();
        let output_address = output_token.address();

        let router_contract = match self.router_contract() {
            Ok(contract) => contract,
            Err(_) => return false, // Return false if the router contract is not created
        };

        let connected_contract = router_contract.connect(self.provider().clone());

        let method_call = connected_contract.method::<_, Vec<U256>>(
            "getAmountsOut",
            (U256::one(), vec![input_address, output_address]),
        );

        let amounts_out = match method_call
            .expect("Failed to execute contract call")
            .call()
            .await
        {
            Ok(result) => result,
            Err(err) => {
                // Handle the error or panic with a custom message
                log::error!("Failed to execute contract call: {:?}", err);
                return false;
            }
        };

        if let Some(output_amount) = amounts_out.get(1) {
            !output_amount.is_zero()
        } else {
            false
        }
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

impl PartialEq for Box<dyn Dex> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.as_ref(), other.as_ref())
    }
}
