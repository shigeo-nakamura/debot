use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, U256},
};

static ERC20_TOKEN_ABI_JSON: &'static [u8] = include_bytes!("../../resources/ERC20TokenABI.json");

#[async_trait]
pub trait Dex: Send + Sync {
    fn token_abi_json(&self) -> &'static [u8] {
        ERC20_TOKEN_ABI_JSON
    }

    fn token_contract(
        &self,
        token_address: Address,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn Error + Send + Sync>> {
        let token_abi = Abi::load(abi_json)?;
        let token_contract = Contract::new(token_address, token_abi, self.get_provider().into());
        Ok(token_contract)
    }

    async fn get_token_decimals(
        &self,
        token_address: Address,
    ) -> Result<u8, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let token_contract = self.token_contract(token_address, self.token_abi_json())?;
        let decimals: u8 = token_contract.method("decimals", ())?.call().await?;
        Ok(decimals)
    }

    async fn get_token_price(
        &self,
        input_token: Address,
        output_token: Address,
        amount: f64,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let input_decimals = self.get_token_decimals(input_token).await?;
        let output_decimals = self.get_token_decimals(output_token).await?;

        let amount_in = U256::from_dec_str(&format!(
            "{:.0}",
            amount * 10f64.powi(input_decimals as i32)
        ))?;

        let router_contract = self.router_contract(self.router_abi_json())?;
        let amounts_out: Vec<U256> = router_contract
            .method::<_, Vec<U256>>(
                "getAmountsOut",
                (amount_in, vec![input_token, output_token]),
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

    async fn swap_tokens(
        &self,
        input_token: Address,
        output_token: Address,
        amount: f64,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn router_contract(
        &self,
        abi_json: &[u8],
    ) -> Result<Contract<Provider<Http>>, Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn clone_box(&self) -> Box<dyn Dex + Send + Sync>;
    fn get_name(&self) -> &str;
    fn router_abi_json(&self) -> &'static [u8];
    fn get_provider(&self) -> Arc<Provider<Http>>;
}
