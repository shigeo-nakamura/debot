use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use dex_connector::{
    BalanceResponse, CreateOrderResponse, DexConnector, DexError, FilledOrder,
    FilledOrdersResponse, LastTradeResponse, OrderSide, TickerResponse,
};
use futures::lock::Mutex;
use num::FromPrimitive;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rust_decimal::{Decimal, RoundingStrategy};

struct OrderBook {
    price: Option<Decimal>,
    size: Decimal,
    order_id: u32,
    partially_filled: bool,
}

struct OrderBooks {
    buy_order_books: Arc<Mutex<Vec<OrderBook>>>,
    sell_order_books: Arc<Mutex<Vec<OrderBook>>>,
}

pub struct DexEmulator<T: DexConnector> {
    dex_connector: T,
    filled_probability: Decimal,
    slippage: Decimal,
    order_books: Arc<Mutex<HashMap<String, OrderBooks>>>,
    order_id_counter: Arc<Mutex<u32>>,
    current_price: Arc<Mutex<HashMap<String, Decimal>>>,
}

impl<T: DexConnector> DexEmulator<T> {
    pub fn new(dex_connector: T, filled_probability: Decimal, slippage: Decimal) -> Self {
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
        current_price: Decimal,
        filled_orders: &mut Vec<(u32, Decimal, Decimal, OrderSide)>,
        is_buy_order: bool,
        rng: &mut impl Rng,
        filled_probability: Decimal,
        slippage: Decimal,
    ) {
        order_books.retain_mut(|order_book| {
            let fill = if order_book.partially_filled {
                order_book.size
            } else if Decimal::from_f64(rng.gen::<f64>()).unwrap() < filled_probability {
                order_book.size
            } else {
                order_book.partially_filled = true;
                Decimal::from_f64(rng.gen::<f64>()).unwrap_or(Decimal::new(1, 0)) * order_book.size
            };

            let always_fill_for_market_order = order_book.price.is_none();
            let adjusted_price = if let Some(price) = order_book.price {
                price
            } else {
                current_price
                    * (Decimal::new(1, 0) + if is_buy_order { slippage } else { -slippage })
            };

            let price_condition = if is_buy_order {
                always_fill_for_market_order || adjusted_price >= current_price
            } else {
                always_fill_for_market_order || adjusted_price <= current_price
            };

            if price_condition && fill > Decimal::new(0, 0) {
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

            order_book.size > Decimal::new(0, 0)
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

    async fn set_leverage(&self, _symbol: &str, _leverage: u32) -> Result<(), DexError> {
        Ok(())
    }

    async fn get_ticker(&self, symbol: &str) -> Result<TickerResponse, DexError> {
        let res = self.dex_connector.get_ticker(symbol).await?;
        let mut price_mutex = self.current_price.lock().await;
        price_mutex.insert(symbol.to_string(), res.price);
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
        let order_books_entry = match order_books.get(symbol) {
            Some(entry) => entry,
            None => {
                log::trace!("Order books not found for symbol: {}", symbol);
                return Ok(FilledOrdersResponse::default());
            }
        };

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
                    trade_id: (order_id + 1000).to_string(),
                    filled_side: Some(side),
                    filled_size: Some(size),
                    filled_value: Some(size * price),
                    filled_fee: Some(Decimal::new(0, 0)),
                    is_rejected: false,
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
        size: Decimal,
        side: OrderSide,
        price: Option<Decimal>,
    ) -> Result<CreateOrderResponse, DexError> {
        let mut order_id_counter = self.order_id_counter.lock().await;
        *order_id_counter += 1;
        let order_id = *order_id_counter;
        drop(order_id_counter); // Explicitly drop the lock

        let size = size.round_dp_with_strategy(5, RoundingStrategy::ToZero);
        let price = match price {
            Some(v) => Some(v.round_dp_with_strategy(5, RoundingStrategy::ToZero)),
            None => None,
        };

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
            ordered_price: price.unwrap_or_default(),
            ordered_size: size,
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id_str: &str) -> Result<(), DexError> {
        let order_id = match order_id_str.parse::<u32>() {
            Ok(id) => id,
            Err(_) => return Err(DexError::Other("Invalid order ID format".to_string())),
        };

        let mut order_books = self.order_books.lock().await;
        if let Some(order_books_entry) = order_books.get_mut(symbol) {
            let mut buy_order_books = order_books_entry.buy_order_books.lock().await;
            buy_order_books.retain(|order_book| order_book.order_id != order_id);

            let mut sell_order_books = order_books_entry.sell_order_books.lock().await;
            sell_order_books.retain(|order_book| order_book.order_id != order_id);
        } else {
            log::warn!("No order books found for symbol: {}", symbol);
        }

        Ok(())
    }

    async fn cancel_all_orders(&self, symbol_option: Option<String>) -> Result<(), DexError> {
        let mut order_books = self.order_books.lock().await;

        match symbol_option {
            Some(symbol) => {
                if let Some(order_books_entry) = order_books.get_mut(&symbol) {
                    let mut buy_order_books = order_books_entry.buy_order_books.lock().await;
                    buy_order_books.clear();

                    let mut sell_order_books = order_books_entry.sell_order_books.lock().await;
                    sell_order_books.clear();
                }
            }
            None => {
                for order_books_entry in order_books.values_mut() {
                    let mut buy_order_books = order_books_entry.buy_order_books.lock().await;
                    buy_order_books.clear();

                    let mut sell_order_books = order_books_entry.sell_order_books.lock().await;
                    sell_order_books.clear();
                }
            }
        }

        Ok(())
    }

    async fn close_all_positions(&self, _symbol: Option<String>) -> Result<(), DexError> {
        Ok(())
    }

    async fn get_last_trades(&self, symbol: &str) -> Result<LastTradeResponse, DexError> {
        self.dex_connector.get_last_trades(symbol).await
    }

    async fn clear_last_trades(&self, symbol: &str) -> Result<(), DexError> {
        self.dex_connector.clear_last_trades(symbol).await
    }
}
