#[derive(Debug)]
pub struct PriceHistory {
    prices: Vec<f64>,
    timestamps: Vec<i64>,
    last_price: f64,
    ema: f64,
    period: usize,
    max_size: usize,
    percentage_drop_threshold: f64,
}

impl PriceHistory {
    pub fn new(period: usize, max_size: usize, percentage_drop_threshold: f64) -> PriceHistory {
        PriceHistory {
            prices: Vec::with_capacity(max_size),
            timestamps: Vec::with_capacity(max_size),
            last_price: 0.0,
            ema: 0.0,
            period: period,
            max_size: max_size,
            percentage_drop_threshold,
        }
    }

    pub fn add_price(&mut self, timestamp: i64, price: f64) {
        if self.prices.len() == self.max_size {
            self.prices.remove(0);
            self.timestamps.remove(0);
        }
        self.prices.push(price);
        self.timestamps.push(timestamp);
        self.update_ema(price);
        self.last_price = price;
    }

    pub fn predict_next_price_ema(&self) -> f64 {
        if self.prices.len() < self.period {
            return self.prices[self.prices.len() - 1];
        }
        2.0 * self.ema - self.prices[self.prices.len() - 1]
    }

    pub fn predict_next_price_regression(&self, next_timestamp: i64) -> f64 {
        let x_mean: f64 = self.timestamps.iter().sum::<i64>() as f64 / self.timestamps.len() as f64;
        let y_mean: f64 = self.prices.iter().sum::<f64>() / self.prices.len() as f64;

        let mut numerator: f64 = 0.0;
        let mut denominator: f64 = 0.0;

        for i in 0..self.prices.len() {
            numerator += (self.timestamps[i] as f64 - x_mean) * (self.prices[i] - y_mean);
            denominator += (self.timestamps[i] as f64 - x_mean).powi(2);
        }

        let slope: f64 = numerator / denominator;
        let intercept: f64 = y_mean - slope * x_mean;

        return slope * next_timestamp as f64 + intercept;
    }

    pub fn predict_next_price_sma(&self) -> f64 {
        if self.prices.len() < self.period {
            return self.prices[self.prices.len() - 1];
        }
        let sum: f64 = self.prices.iter().sum();
        let sma = sum / self.period as f64;
        sma
    }

    pub fn predict_next_price_macd(&self) -> f64 {
        if self.prices.len() < self.period {
            return self.prices[self.prices.len() - 1];
        }
        let short_ema = self.calculate_ema(self.prices.len(), self.period);
        let long_ema = self.calculate_ema(self.prices.len(), self.period * 2);
        let macd_line = short_ema - long_ema;
        let signal_line = self.calculate_ema(self.prices.len(), self.period / 2);
        let histogram = macd_line - signal_line;
        let predicted_price = self.prices[self.prices.len() - 1] + histogram;
        predicted_price
    }

    pub fn calculate_std_dev(&self) -> f64 {
        let mean = self.prices.iter().sum::<f64>() / self.prices.len() as f64;
        let variance =
            self.prices.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / self.prices.len() as f64;
        variance.sqrt()
    }

    pub fn is_flash_crash(&self) -> bool {
        if self.last_price < self.ema * (1.0 - self.percentage_drop_threshold / 100.0) {
            return true;
        }
        false
    }

    fn update_ema(&mut self, price: f64) {
        if self.prices.len() == 1 {
            self.ema = price;
        } else {
            let weight = 2.0 / (self.period as f64 + 1.0);
            self.ema = (price - self.ema) * weight + self.ema;
        }
    }

    fn calculate_ema(&self, current_index: usize, period: usize) -> f64 {
        let mut ema = self.prices[current_index - period];
        let multiplier = 2.0 / (period as f64 + 1.0);
        for i in (current_index - period + 1)..current_index {
            ema = (self.prices[i] - ema) * multiplier + ema;
        }
        ema
    }
}
