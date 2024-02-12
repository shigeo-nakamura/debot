use async_trait::async_trait;
use dex_connector::{
    BalanceResponse, CreateOrderResponse, DexConnector, DexError, FilledOrdersResponse, OrderSide,
    RabbitxConnector, TickerResponse,
};

use crate::config::get_rabbitx_config_from_env;

use super::{dex_emulator::DexEmulator, fund_config::RABBITX_TOKEN_LIST};

pub struct DexConnectorBox {
    inner: Box<dyn DexConnector>,
}

impl DexConnectorBox {
    pub async fn create(
        dex_name: &str,
        rest_endpoint: &str,
        web_socket_endpoint: &str,
        dry_run: bool,
    ) -> Result<Self, DexError> {
        match dex_name {
            "rabbitx" => {
                let rabbitx_config = match get_rabbitx_config_from_env().await {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(DexError::Other("Some env vars are missing".to_string()));
                    }
                };

                let market_ids: Vec<String> =
                    RABBITX_TOKEN_LIST.iter().map(|&s| s.to_string()).collect();
                let connector = RabbitxConnector::new(
                    rest_endpoint,
                    web_socket_endpoint,
                    &rabbitx_config.profile_id,
                    &rabbitx_config.api_key,
                    &rabbitx_config.public_jwt,
                    &rabbitx_config.refresh_token,
                    &rabbitx_config.secret,
                    &rabbitx_config.private_jwt,
                    &market_ids,
                )
                .await?;
                connector.start().await?;

                if dry_run {
                    let dex_emulator = DexEmulator::new(connector, 0.9, 0.01);
                    Ok(DexConnectorBox {
                        inner: Box::new(dex_emulator),
                    })
                } else {
                    Ok(DexConnectorBox {
                        inner: Box::new(connector),
                    })
                }
            }
            _ => Err(DexError::Other("Unsupported dex".to_owned())),
        }
    }
}

#[async_trait]
impl DexConnector for DexConnectorBox {
    async fn start(&self) -> Result<(), DexError> {
        self.inner.start().await
    }

    async fn stop(&self) -> Result<(), DexError> {
        self.inner.stop().await
    }

    async fn set_leverage(&self, symbol: &str, leverage: &str) -> Result<(), DexError> {
        self.inner.set_leverage(symbol, leverage).await
    }

    async fn get_ticker(&self, symbol: &str) -> Result<TickerResponse, DexError> {
        self.inner.get_ticker(symbol).await
    }

    async fn get_filled_orders(&self, symbol: &str) -> Result<FilledOrdersResponse, DexError> {
        self.inner.get_filled_orders(symbol).await
    }

    async fn get_balance(&self) -> Result<BalanceResponse, DexError> {
        self.inner.get_balance().await
    }

    async fn clear_filled_order(&self, symbol: &str, order_id: &str) -> Result<(), DexError> {
        self.inner.clear_filled_order(symbol, order_id).await
    }

    async fn create_order(
        &self,
        symbol: &str,
        size: &str,
        side: OrderSide,
        price: Option<String>,
    ) -> Result<CreateOrderResponse, DexError> {
        self.inner.create_order(symbol, size, side, price).await
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<(), DexError> {
        self.inner.cancel_order(symbol, order_id).await
    }

    async fn cancel_all_orders(&self, symbol: Option<String>) -> Result<(), DexError> {
        self.inner.cancel_all_orders(symbol).await
    }

    async fn close_all_positions(&self, symbol: Option<String>) -> Result<(), DexError> {
        self.inner.close_all_positions(symbol).await
    }
}
