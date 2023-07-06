// open_position.rs

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TradePosition {
    pub id: Option<u32>,
    pub token_name: String,
    pub fund_name: String,
    pub open_time: i64,
    pub open_time_str: String,
    pub close_time_str: String,
    pub average_buy_price: f64,
    pub take_profit_price: f64,
    pub cut_loss_price: f64,
    pub sold_price: Option<f64>,
    pub sold_amount: Option<f64>,
    pub amount: f64,
    pub amount_in_base_token: f64,
    pub realized_pnl: Option<f64>,
}

impl TradePosition {
    pub fn new(
        token_name: &str,
        fund_name: &str,
        average_buy_price: f64,
        take_profit_price: f64,
        cut_loss_price: f64,
        amount: f64,
        amount_in_base_token: f64,
    ) -> Self {
        log::debug!(
            "Created new open position for token: {}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}",
            token_name, average_buy_price, take_profit_price, cut_loss_price,
        );

        let open_time = chrono::Utc::now().timestamp();

        Self {
            id: None,
            token_name: token_name.to_owned(),
            fund_name: fund_name.to_owned(),
            open_time,
            open_time_str: Self::get_datetime_string(open_time),
            close_time_str: String::new(),
            average_buy_price,
            take_profit_price,
            cut_loss_price,
            sold_price: None,
            sold_amount: None,
            amount,
            amount_in_base_token,
            realized_pnl: None,
        }
    }

    pub fn do_take_profit(&self, sell_price: f64) -> bool {
        sell_price >= self.take_profit_price
    }

    pub fn do_cut_loss(&self, sell_price: f64) -> bool {
        sell_price <= self.cut_loss_price
    }

    fn update(&mut self, average_price: f64, amount: f64, is_buy: bool) {
        if is_buy {
            self.amount += amount;
            self.average_buy_price = (self.average_buy_price * self.amount
                + average_price * amount)
                / (self.amount + amount);

            log::info!(
            "Updated open position for token: {}, amount: {:6.3}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}",
            self.token_name,
            self.amount,
            self.average_buy_price,
            self.take_profit_price,
            self.cut_loss_price,
        );
        } else {
            self.amount -= amount;
            self.sold_price = Some(average_price);
            self.sold_amount = Some(amount);
            let pnl = (average_price - self.average_buy_price) * amount;
            self.realized_pnl = Some(pnl);
            self.close_time_str = Self::get_datetime_string(chrono::Utc::now().timestamp());

            log::info!(
                "Cloes the position for token: {}, amount: {:6.3}, PNL: {:6.3}",
                self.token_name,
                amount,
                pnl
            );
        }
    }

    pub fn del(&mut self, sold_price: f64, amount: f64) {
        self.update(sold_price, amount, false);
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
        self.open_time_str = Self::get_datetime_string(self.open_time);

        self.amount_in_base_token += amount_in_base_token;

        self.take_profit_price = (self.take_profit_price * self.amount
            + take_profit_price * amount)
            / (self.amount + amount);
        self.cut_loss_price =
            (self.cut_loss_price * self.amount + cut_loss_price * amount) / (self.amount + amount);

        self.update(average_price, amount, true);
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
            self.cut_loss_price,
            self.amount
        );
    }

    fn get_datetime_string(time: i64) -> String {
        let naive_datetime =
            chrono::NaiveDateTime::from_timestamp_opt(time, 0).expect("Invalid timestamp");
        naive_datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}
