#[derive(Debug)]
pub struct PriceHistory {
    prices: Vec<f64>,
    timestamps: Vec<i64>,
    ema: f64,
    period: usize,
    max_size: usize,
}

impl PriceHistory {
    pub fn new(period: usize, max_size: usize) -> PriceHistory {
        PriceHistory {
            prices: Vec::with_capacity(max_size),
            timestamps: Vec::with_capacity(max_size),
            ema: 0.0,
            period: period,
            max_size: max_size,
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
    }

    fn update_ema(&mut self, price: f64) {
        if self.prices.len() == 1 {
            self.ema = price;
        } else {
            let weight = 2.0 / (self.period as f64 + 1.0);
            self.ema = (price - self.ema) * weight + self.ema;
        }
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
}
