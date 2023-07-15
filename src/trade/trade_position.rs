// open_position.rs

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::utils::{DateTimeUtils, ToDateTimeString};

use super::HasId;

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
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Open,
    Closed,
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
        atr: f64,
    ) -> Self {
        log::debug!(
            "Created new open position for token: {}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}",
            token_name, average_buy_price, take_profit_price, cut_loss_price,
        );

        let open_time = chrono::Utc::now().timestamp();

        let modified_cut_loss_price = match cut_loss_strategy {
            CutLossStrategy::FixedThreshold => cut_loss_price,
            CutLossStrategy::ATRStop => average_buy_price - atr,
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
            initial_cut_loss_price: cut_loss_price,
            trailing_distance: take_profit_price - average_buy_price,
            sold_price: None,
            sold_amount: None,
            amount,
            amount_in_base_token,
            realized_pnl: None,
        }
    }

    pub fn do_take_profit(&self, sell_price: f64) -> bool {
        match self.take_profit_strategy {
            TakeProfitStrategy::FixedThreshold => sell_price >= self.take_profit_price,
            TakeProfitStrategy::TrailingStop => {
                let current_distance = sell_price - self.average_buy_price;
                if current_distance > self.trailing_distance {
                    let cut_loss_price = sell_price - self.trailing_distance;
                    let mut cut_loss_price_self = self.cut_loss_price.lock().unwrap();
                    if cut_loss_price > *cut_loss_price_self {
                        *cut_loss_price_self = cut_loss_price;
                    }
                }
                false
            }
        }
    }

    pub fn do_cut_loss(&self, sell_price: f64) -> bool {
        sell_price <= *self.cut_loss_price.lock().unwrap()
    }

    fn update(&mut self, average_price: f64, amount: f64) {
        if self.state == State::Open {
            self.amount += amount;
            self.average_buy_price = (self.average_buy_price * self.amount
                + average_price * amount)
                / (self.amount + amount);
            if amount == self.amount {
                log::info!("Open new position :{:?}", self);
            } else {
                log::info!("Updated open position :{:?}", self);
            }
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
            "ID: {}, Token: {} PNL = {:6.3}, current_price: {:6.3}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}, amount: {:6.6}",
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
