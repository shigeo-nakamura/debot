// dex.rs

use crate::token::Token;
use async_trait::async_trait;
use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, U256},
};
use std::{error::Error, sync::Arc};

#[derive(Debug, Clone)]
pub struct BaseDex {
    pub router_address: Address,
    pub provider: Arc<Provider<Http>>,
}

impl BaseDex {
    pub fn router_contract(
        &self,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn Error + Send + Sync>> {
        let router_abi = Abi::load(abi_json)?;
        let router_contract = Contract::new(
            self.router_address,
            router_abi,
            self.provider.clone().into(),
        );
        Ok(router_contract)
    }

    pub fn get_provider(&self) -> Arc<Provider<Http>> {
        self.provider.clone()
    }
}

#[async_trait]
pub trait Dex: Send + Sync {
    async fn get_token_price(
        &self,
        input_token: &dyn Token,
        output_token: &dyn Token,
        amount: f64,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let input_address = input_token.address();
        let output_address = output_token.address();

        let input_decimals = input_token.decimals().await?;
        let output_decimals = output_token.decimals().await?;

        let amount_in = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(input_decimals as i32)
        ))?;

        let router_contract = self.router_contract(self.router_abi_json())?;
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
            self.get_name(),
            amount_in,
            output_amount,
            price_f64
        );

        Ok(price_f64)
    }

    fn router_contract(
        &self,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn clone_box(&self) -> Box<dyn Dex + Send + Sync>;
    fn get_name(&self) -> &str;
    fn router_abi_json(&self) -> &'static [u8];
    fn get_provider(&self) -> Arc<Provider<Http>>;
}
