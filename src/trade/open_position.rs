// open_position.rs

#[derive(Clone, Debug)]
pub struct OpenPosition {
    pub transaction_id: u32,
    pub open_time: i64,
    pub average_price: f64,
    pub take_profit_price: f64,
    pub cut_loss_price: f64,
    pub amount: f64,
    pub amount_in_base_token: f64,
    realized_pnl: f64,
}

impl OpenPosition {
    pub fn new(
        transaction_id: u32,
        token_name: &str,
        average_price: f64,
        take_profit_price: f64,
        cut_loss_price: f64,
        amount: f64,
        amount_in_base_token: f64,
    ) -> Self {
        log::debug!(
            "Created new open position for token: {}, average_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}",
            token_name, average_price, take_profit_price, cut_loss_price,
        );

        Self {
            transaction_id,
            open_time: chrono::Utc::now().timestamp(),
            average_price,
            take_profit_price,
            cut_loss_price,
            amount,
            amount_in_base_token,
            realized_pnl: 0.0,
        }
    }

    pub fn do_take_profit(&self) -> bool {
        self.average_price >= self.take_profit_price
    }

    pub fn do_cut_loss(&self) -> bool {
        self.average_price <= self.cut_loss_price
    }

    fn update(&mut self, token_name: &str, average_price: f64, amount: f64) {
        if self.amount + amount == 0.0 {
            self.realized_pnl = self.average_price * self.amount - average_price * amount;
        } else {
            self.average_price = (self.average_price * self.amount + average_price * amount)
                / (self.amount + amount);
        }

        self.amount += amount;

        log::debug!(
            "Updated open position for token: {}, amount: {:6.3}, average price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}",
            token_name,
            self.amount,
            self.average_price,
            self.take_profit_price,
            self.cut_loss_price,
        );
    }

    pub fn del(&mut self, token_name: &str, average_price: f64, amount: f64) {
        self.update(token_name, average_price, amount);
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
        let pnl = (current_price - self.average_price) * self.amount;
        log::debug!(
            "{} PNL = {:6.3}, current_price: {:6.3}, average_price: {:6.3}, take_profit_price: {:6.3}, cut_loss_price: {:6.3}, amount: {:6.6}",
            token_name,
            pnl,
            current_price,
            self.average_price,
            self.take_profit_price,
            self.cut_loss_price,
            self.amount
        );
    }
}
