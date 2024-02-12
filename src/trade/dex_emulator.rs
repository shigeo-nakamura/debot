use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use dex_connector::{
    BalanceResponse, CreateOrderResponse, DexConnector, DexError, FilledOrder,
    FilledOrdersResponse, OrderSide, TickerResponse,
};
use futures::lock::Mutex;
use rand::{rngs::StdRng, Rng, SeedableRng};

struct OrderBook {
    price: Option<f64>,
    size: f64,
    order_id: u32,
    partially_filled: bool,
}

struct OrderBooks {
    buy_order_books: Arc<Mutex<Vec<OrderBook>>>,
    sell_order_books: Arc<Mutex<Vec<OrderBook>>>,
}

pub struct DexEmulator<T: DexConnector> {
    dex_connector: T,
    filled_probability: f64,
    slippage: f64,
    order_books: Arc<Mutex<HashMap<String, OrderBooks>>>,
    order_id_counter: Arc<Mutex<u32>>,
    current_price: Arc<Mutex<HashMap<String, f64>>>,
}

impl<T: DexConnector> DexEmulator<T> {
    pub fn new(dex_connector: T, filled_probability: f64, slippage: f64) -> Self {
        log::info!("Use DexEmulator");
        let mut rng = rand::thread_rng();
        let order_id_counter = rng.gen_range(1..=std::u32::MAX);

        DexEmulator {
            dex_connector,
            filled_probability,
            slippage,
            order_books: Arc::new(Mutex::new(HashMap::new())),
            order_id_counter: Arc::new(Mutex::new(order_id_counter)),
            current_price: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn process_order_book(
        order_books: &mut Vec<OrderBook>,
        current_price: f64,
        filled_orders: &mut Vec<(u32, f64, f64, OrderSide)>,
        is_buy_order: bool,
        rng: &mut impl Rng,
        filled_probability: f64,
        slippage: f64,
    ) {
        order_books.retain_mut(|order_book| {
            let fill = if order_book.partially_filled {
                order_book.size
            } else if rng.gen::<f64>() < filled_probability {
                order_book.size
            } else {
                order_book.partially_filled = true;
                rng.gen_range(0.0..order_book.size)
            };

            let price_condition = if is_buy_order {
                order_book.price.map_or(true, |p| p >= current_price)
            } else {
                order_book.price.map_or(true, |p| p <= current_price)
            };

            if price_condition && fill > 0.0 {
                let adjusted_price = order_book.price.unwrap_or_else(|| {
                    current_price * (1.0 + if is_buy_order { slippage } else { -slippage })
                });
                filled_orders.push((
                    order_book.order_id,
                    fill,
                    adjusted_price,
                    if is_buy_order {
                        OrderSide::Long
                    } else {
                        OrderSide::Short
                    },
                ));
                order_book.size -= fill;
            }

            order_book.size > 0.0
        });
    }
}

#[async_trait]
impl<T: DexConnector> DexConnector for DexEmulator<T> {
    async fn start(&self) -> Result<(), DexError> {
        self.dex_connector.start().await
    }

    async fn stop(&self) -> Result<(), DexError> {
        self.dex_connector.stop().await
    }

    async fn set_leverage(&self, _symbol: &str, _leverage: &str) -> Result<(), DexError> {
        Ok(())
    }

    async fn get_ticker(&self, symbol: &str) -> Result<TickerResponse, DexError> {
        let res = self.dex_connector.get_ticker(symbol).await?;
        let mut price_mutex = self.current_price.lock().await;
        price_mutex.entry(symbol.to_string()).or_insert(res.price);
        Ok(res)
    }

    async fn get_filled_orders(&self, symbol: &str) -> Result<FilledOrdersResponse, DexError> {
        let current_price = {
            let price_mutex = self.current_price.lock().await;
            match price_mutex.get(symbol) {
                Some(v) => *v,
                None => {
                    log::info!("get_filled_orders: price for {} is not known yet", symbol);
                    return Ok(FilledOrdersResponse::default());
                }
            }
        };

        let mut rng = StdRng::from_entropy();
        let order_books = self.order_books.lock().await;
        let order_books_entry = order_books.get(symbol).ok_or(DexError::Other(format!(
            "Order books not found for symbol: {}",
            symbol
        )))?;

        let mut filled_orders = Vec::new();

        // Process buy order books
        {
            let mut buy_order_books = order_books_entry.buy_order_books.lock().await;
            Self::process_order_book(
                &mut buy_order_books,
                current_price,
                &mut filled_orders,
                true, // is_buy_order
                &mut rng,
                self.filled_probability,
                self.slippage,
            )
            .await;
        }

        // Process sell order books
        {
            let mut sell_order_books = order_books_entry.sell_order_books.lock().await;
            Self::process_order_book(
                &mut sell_order_books,
                current_price,
                &mut filled_orders,
                false, // is_buy_order
                &mut rng,
                self.filled_probability,
                self.slippage,
            )
            .await;
        }

        Ok(FilledOrdersResponse {
            orders: filled_orders
                .into_iter()
                .map(|(order_id, size, price, side)| FilledOrder {
                    order_id: order_id.to_string(),
                    filled_side: side,
                    filled_size: size,
                    filled_value: size * price,
                    filled_fee: 0.0,
                })
                .collect(),
        })
    }

    async fn get_balance(&self) -> Result<BalanceResponse, DexError> {
        self.dex_connector.get_balance().await
    }

    async fn clear_filled_order(&self, _symbol: &str, _order_id: &str) -> Result<(), DexError> {
        Ok(())
    }

    async fn create_order(
        &self,
        symbol: &str,
        size: &str,
        side: OrderSide,
        price: Option<String>,
    ) -> Result<CreateOrderResponse, DexError> {
        let price = price
            .as_ref()
            .map(|p| p.parse::<f64>())
            .transpose()
            .map_err(|e| DexError::Other(format!("{:?}", e)))?;
        let size = size
            .parse::<f64>()
            .map_err(|e| DexError::Other(format!("{:?}", e)))?;

        let mut order_id_counter = self.order_id_counter.lock().await;
        *order_id_counter += 1;
        let order_id = *order_id_counter;
        drop(order_id_counter); // Explicitly drop the lock

        let order_book = OrderBook {
            price,
            size,
            order_id,
            partially_filled: false,
        };

        let mut order_books = self.order_books.lock().await;
        let order_books_entry =
            order_books
                .entry(symbol.to_string())
                .or_insert_with(|| OrderBooks {
                    buy_order_books: Arc::new(Mutex::new(Vec::new())),
                    sell_order_books: Arc::new(Mutex::new(Vec::new())),
                });

        if side == OrderSide::Long {
            let mut buy_order_books = order_books_entry.buy_order_books.lock().await;
            buy_order_books.push(order_book);
        } else {
            // Assuming side can only be Buy or Sell
            let mut sell_order_books = order_books_entry.sell_order_books.lock().await;
            sell_order_books.push(order_book);
        }

        Ok(CreateOrderResponse {
            order_id: order_id.to_string(),
        })
    }

    async fn cancel_order(&self, _symbol: &str, _order_id: &str) -> Result<(), DexError> {
        Ok(())
    }

    async fn cancel_all_orders(&self, _symbol: Option<String>) -> Result<(), DexError> {
        Ok(())
    }

    async fn close_all_positions(&self, _symbol: Option<String>) -> Result<(), DexError> {
        Ok(())
    }
}
