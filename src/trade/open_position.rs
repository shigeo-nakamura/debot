// open_position.rs

#[derive(Clone, Debug)]
pub struct OpenPosition {
    pub transaction_log_id: Option<u32>,
    pub open_time: i64,
    pub close_time: Option<i64>,
    pub average_buy_price: f64,
    pub take_profit_price: f64,
    pub cut_loss_price: f64,
    pub sold_price: Option<f64>,
    pub amount: f64,
    pub amount_in_base_token: f64,
    pub realized_pnl: Option<f64>,
}

impl OpenPosition {
    pub fn new(
        token_name: &str,
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

        Self {
            transaction_log_id: None,
            open_time: chrono::Utc::now().timestamp(),
            close_time: None,
            average_buy_price,
            take_profit_price,
            cut_loss_price,
            sold_price: None,
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

    fn update(&mut self, token_name: &str, average_price: f64, amount: f64) {
        self.amount += amount;

        if self.amount == 0.0 {
            self.sold_price = Some(average_price);
            let pnl = (average_price - self.average_buy_price) * amount;
            self.realized_pnl = Some(pnl);
            self.close_time = Some(chrono::Utc::now().timestamp());

            log::debug!(
                "Cloes the position for token: {}, amount: {:6.3}, PNL: {:6.3}",
                token_name,
                amount,
                pnl
            );
        } else {
            self.average_buy_price = (self.average_buy_price * self.amount
                + average_price * amount)
                / (self.amount + amount);

            log::debug!(
            "Updated open position for token: {}, amount: {:6.3}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}",
            token_name,
            self.amount,
            self.average_buy_price,
            self.take_profit_price,
            self.cut_loss_price,
        );
        }
    }

    pub fn del(&mut self, token_name: &str, sold_price: f64, amount: f64) {
        let amount = amount * -1.0;
        self.update(token_name, sold_price, amount);
    }

    pub fn add(
        &mut self,
        token_name: &str,
        average_price: f64,
        take_profit_price: f64,
        cut_loss_price: f64,
        amount: f64,
    ) {
        self.open_time = chrono::Utc::now().timestamp();

        self.take_profit_price = (self.take_profit_price * self.amount
            + take_profit_price * amount)
            / (self.amount + amount);
        self.cut_loss_price =
            (self.cut_loss_price * self.amount + cut_loss_price * amount) / (self.amount + amount);

        self.update(token_name, average_price, amount);
    }

    pub fn print_info(&self, token_name: &str, current_price: f64) {
        let pnl = (current_price - self.average_buy_price) * self.amount;
        let transaction_log_id = match self.transaction_log_id {
            Some(id) => id,
            None => 0,
        };

        log::debug!(
            "ID: {}, Token: {} PNL = {:6.3}, current_price: {:6.3}, average_buy_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}, amount: {:6.6}",
            transaction_log_id,
            token_name,
            pnl,
            current_price,
            self.average_buy_price,
            self.take_profit_price,
            self.cut_loss_price,
            self.amount
        );
    }

    pub fn get_datetime_string(&self, time: i64) -> String {
        let naive_datetime =
            chrono::NaiveDateTime::from_timestamp_opt(time, 0).expect("Invalid timestamp");
        naive_datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}
