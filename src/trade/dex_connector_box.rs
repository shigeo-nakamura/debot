use async_trait::async_trait;
use dex_connector::{
    BalanceResponse, CreateOrderResponse, DexConnector, DexError, FilledOrdersResponse,
    HyperliquidConnector, OrderSide, TickerResponse,
};
use rust_decimal::Decimal;

use super::dex_emulator::DexEmulator;
use crate::config::get_hyperliquid_config_from_env;
use lazy_static::lazy_static;
use std::env;

lazy_static! {
    static ref FILLED_PROBABILITY_IN_EMULATION: Decimal = {
        match env::var("FILLED_PROBABILITY_IN_EMULATION") {
            Ok(val) => val.parse::<Decimal>().unwrap_or(Decimal::new(1, 0)),
            Err(_) => Decimal::new(1, 0),
        }
    };
}

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
            "hyperliquid" => {
                let hyperliquid_config = match get_hyperliquid_config_from_env().await {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(DexError::Other("Some env vars are missing".to_string()));
                    }
                };

                let connector = HyperliquidConnector::new(
                    rest_endpoint,
                    web_socket_endpoint,
                    &hyperliquid_config.agent_private_key,
                    &hyperliquid_config.evm_wallet_address,
                    hyperliquid_config.vault_address,
                )
                .await?;

                if dry_run {
                    let dex_emulator = DexEmulator::new(
                        connector,
                        *FILLED_PROBABILITY_IN_EMULATION,
                        Decimal::new(5, 3),
                    );
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

    async fn restart(&self) -> Result<(), DexError> {
        self.inner.restart().await
    }

    async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<(), DexError> {
        self.inner.set_leverage(symbol, leverage).await
    }

    async fn get_ticker(
        &self,
        symbol: &str,
        test_price: Option<Decimal>,
    ) -> Result<TickerResponse, DexError> {
        self.inner.get_ticker(symbol, test_price).await
    }

    async fn get_filled_orders(&self, symbol: &str) -> Result<FilledOrdersResponse, DexError> {
        self.inner.get_filled_orders(symbol).await
    }

    async fn get_balance(&self) -> Result<BalanceResponse, DexError> {
        self.inner.get_balance().await
    }

    async fn clear_filled_order(&self, symbol: &str, trade_id: &str) -> Result<(), DexError> {
        self.inner.clear_filled_order(symbol, trade_id).await
    }

    async fn clear_all_filled_order(&self) -> Result<(), DexError> {
        self.inner.clear_all_filled_order().await
    }

    async fn create_order(
        &self,
        symbol: &str,
        size: Decimal,
        side: OrderSide,
        price: Option<Decimal>,
        spread: Option<i64>,
    ) -> Result<CreateOrderResponse, DexError> {
        self.inner
            .create_order(symbol, size, side, price, spread)
            .await
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

    async fn clear_last_trades(&self, symbol: &str) -> Result<(), DexError> {
        self.inner.clear_last_trades(symbol).await
    }
}
