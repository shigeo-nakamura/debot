// open_position.rs

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::utils::{DateTimeUtils, ToDateTimeString};

use super::{abstract_trader::ReasonForSell, HasId};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum TakeProfitStrategy {
    #[default]
    FixedThreshold,
    TrailingStop,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum CutLossStrategy {
    #[default]
    FixedThreshold,
    ATRStop,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TradePosition {
    pub id: Option<u32>,
    pub take_profit_strategy: TakeProfitStrategy,
    pub cut_loss_strategy: CutLossStrategy,
    pub state: State,
    pub token_name: String,
    pub fund_name: String,
    pub open_time: i64,
    pub open_time_str: String,
    pub close_time_str: String,
    pub average_buy_price: f64,
    pub take_profit_price: f64,
    #[serde(skip)]
    pub cut_loss_price: Arc<std::sync::Mutex<f64>>,
    pub initial_cut_loss_price: f64,
    pub trailing_distance: f64,
    pub sold_price: Option<f64>,
    pub sold_amount: Option<f64>,
    pub amount: f64,
    pub amount_in_base_token: f64,
    pub realized_pnl: Option<f64>,
    pub atr: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Open,
    CutLoss,
    TakeProfit,
    Liquidated,
    Expired,
}

impl HasId for TradePosition {
    fn id(&self) -> Option<u32> {
        self.id
    }
}

impl TradePosition {
    pub fn new(
        token_name: &str,
        fund_name: &str,
        take_profit_strategy: TakeProfitStrategy,
        cut_loss_strategy: CutLossStrategy,
        average_buy_price: f64,
        take_profit_price: f64,
        cut_loss_price: f64,
        amount: f64,
        amount_in_base_token: f64,
        atr: Option<f64>,
    ) -> Self {
        log::debug!(
            "Created new open position for token: {}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}, atr:{:?}",
            token_name, average_buy_price, take_profit_price, cut_loss_price, atr
        );

        let open_time = chrono::Utc::now().timestamp();

        let modified_cut_loss_price = match cut_loss_strategy {
            CutLossStrategy::FixedThreshold => cut_loss_price,
            CutLossStrategy::ATRStop => {
                let distance = average_buy_price - cut_loss_price;
                let cut_loss_distance = match atr {
                    Some(atr) => f64::max(atr, distance),
                    None => distance,
                };
                average_buy_price - cut_loss_distance
            }
        };

        Self {
            id: None,
            take_profit_strategy,
            cut_loss_strategy,
            state: State::Open,
            token_name: token_name.to_owned(),
            fund_name: fund_name.to_owned(),
            open_time,
            open_time_str: open_time.to_datetime_string(),
            close_time_str: String::new(),
            average_buy_price,
            take_profit_price,
            cut_loss_price: Arc::new(std::sync::Mutex::new(modified_cut_loss_price)),
            initial_cut_loss_price: modified_cut_loss_price,
            trailing_distance: take_profit_price - average_buy_price,
            sold_price: None,
            sold_amount: None,
            amount,
            amount_in_base_token,
            realized_pnl: None,
            atr: atr,
        }
    }

    fn should_take_profit_fixed_threshold(&self, sell_price: f64) -> bool {
        sell_price >= self.take_profit_price
    }

    // Adjusts cut loss price and returns false. The cut loss price is adjusted
    // based on the sell price and the trailing distance.
    fn should_take_profit_trailing_stop(&self, sell_price: f64) -> bool {
        let current_distance = if sell_price - self.average_buy_price > 0.0 {
            sell_price - self.average_buy_price
        } else {
            sell_price - *self.cut_loss_price.lock().unwrap()
        };

        if current_distance > self.trailing_distance {
            let cut_loss_price = sell_price - self.trailing_distance;
            let mut cut_loss_price_self = self.cut_loss_price.lock().unwrap();
            if cut_loss_price > *cut_loss_price_self {
                *cut_loss_price_self = cut_loss_price;
            }
        }

        false
    }

    pub fn should_close(
        &self,
        sell_price: f64,
        max_holding_interval: i64,
    ) -> Option<ReasonForSell> {
        if self.should_take_profit(sell_price) {
            return Some(ReasonForSell::TakeProfit);
        }
        self.should_cut_loss(sell_price, max_holding_interval)
    }

    fn should_take_profit(&self, sell_price: f64) -> bool {
        match self.take_profit_strategy {
            TakeProfitStrategy::FixedThreshold => {
                self.should_take_profit_fixed_threshold(sell_price)
            }
            TakeProfitStrategy::TrailingStop => self.should_take_profit_trailing_stop(sell_price),
        }
    }
    fn should_cut_loss(&self, sell_price: f64, max_holding_interval: i64) -> Option<ReasonForSell> {
        let current_time = chrono::Utc::now().timestamp();
        let holding_interval = current_time - self.open_time;
        let cut_loss_price = *self.cut_loss_price.lock().unwrap();

        if sell_price < cut_loss_price {
            if sell_price > self.average_buy_price {
                return Some(ReasonForSell::TakeProfit);
            } else {
                return Some(ReasonForSell::CutLoss);
            }
        }

        if self.take_profit_price < cut_loss_price {
            return None;
        }

        if holding_interval > max_holding_interval * 2 {
            return Some(ReasonForSell::Expired);
        } else if holding_interval > max_holding_interval {
            if sell_price > self.average_buy_price {
                return Some(ReasonForSell::TakeProfit);
            }
        }
        None
    }

    fn update(&mut self, average_price: f64, amount: f64) {
        if self.state == State::Open {
            self.amount += amount;
            self.average_buy_price = (self.average_buy_price * self.amount
                + average_price * amount)
                / (self.amount + amount);
            log::info!("Updated open position :{:?}", self);
        } else {
            self.amount -= amount;
            self.sold_price = Some(average_price);
            self.sold_amount = Some(amount);
            let pnl = (average_price - self.average_buy_price) * amount;
            self.realized_pnl = Some(pnl);
            self.close_time_str = DateTimeUtils::get_current_datetime_string();

            log::info!("Cloes the position: {:?}", self);
        }
    }

    pub fn del(&mut self, sold_price: f64, amount: f64, state: State) {
        self.state = state;
        self.update(sold_price, amount);
    }

    pub fn add(
        &mut self,
        average_price: f64,
        take_profit_price: f64,
        cut_loss_price: f64,
        amount: f64,
        amount_in_base_token: f64,
    ) {
        self.open_time = chrono::Utc::now().timestamp();
        self.open_time_str = self.open_time.to_datetime_string();

        self.amount_in_base_token += amount_in_base_token;

        self.take_profit_price = (self.take_profit_price * self.amount
            + take_profit_price * amount)
            / (self.amount + amount);

        let mut self_cut_loss_price = self.cut_loss_price.lock().unwrap();
        *self_cut_loss_price =
            (*self_cut_loss_price * self.amount + cut_loss_price * amount) / (self.amount + amount);
        drop(self_cut_loss_price);

        self.update(average_price, amount);
    }

    pub fn print_info(&self, current_price: f64) {
        let pnl = (current_price - self.average_buy_price) * self.amount;
        let id = match self.id {
            Some(id) => id,
            None => 0,
        };

        log::debug!(
            "ID: {}, Token: {} PNL: {:6.3}, current: {:6.3}, average_buy: {:6.3}, take_profit: {:6.3}, cut_loss: {:6.3}, amount: {:6.6}",
            id,
            self.token_name,
            pnl,
            current_price,
            self.average_buy_price,
            self.take_profit_price,
            *self.cut_loss_price.lock().unwrap(),
            self.amount
        );
    }
}
