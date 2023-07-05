// price_history.rs

const SIGNAL_PERIOD: usize = 9;
const MACD_THRESHOLD: f64 = 0.1;

// RSI thresholds
const RSI_OVERBOUGHT: f64 = 70.0;
const RSI_OVERSOLD: f64 = 30.0;

// Threshold for detecting flash crash based on RSI
const RSI_FLASH_CRASH: f64 = 85.0;

#[derive(Debug, Clone)]
pub struct PriceHistory {
    prices: Vec<f64>,
    timestamps: Vec<i64>,
    last_price: f64,
    ema_short: f64,
    ema_medium: f64,
    ema_long: f64,
    short_period: usize,
    medium_period: usize,
    long_period: usize,
    max_size: usize,
    flash_crash_threshold: f64,
}

#[derive(Clone, Copy)]
pub enum TradingStrategy {
    TrendFollowing,
    MeanReversion,
    Contrarian,
}

impl PriceHistory {
    pub fn new(
        short_period: usize,
        medium_period: usize,
        long_period: usize,
        max_size: usize,
        flash_crash_threshold: f64,
    ) -> PriceHistory {
        PriceHistory {
            prices: Vec::with_capacity(max_size),
            timestamps: Vec::with_capacity(max_size),
            last_price: 0.0,
            ema_short: 0.0,
            ema_medium: 0.0,
            ema_long: 0.0,
            short_period,
            medium_period,
            long_period,
            max_size: max_size,
            flash_crash_threshold,
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

    pub fn predict_next_price_ema(&self, period: usize) -> f64 {
        let predict = |len, ema| {
            if self.prices.len() >= len {
                2.0 * ema - self.prices[self.prices.len() - 1]
            } else {
                self.prices[self.prices.len() - 1]
            }
        };

        match period {
            x if x == self.short_period => predict(self.short_period, self.ema_short),
            x if x == self.medium_period => predict(self.medium_period, self.ema_medium),
            x if x == self.long_period => predict(self.long_period, self.ema_long),
            _ => {
                log::error!("Invalid period");
                predict(self.short_period, self.ema_short)
            }
        }
    }

    pub fn predict_next_price_sma(&self, period: usize) -> f64 {
        self.calculate_sma(period)
    }

    pub fn predict_next_price_macd(&self) -> f64 {
        if self.prices.len() < self.short_period + 1 {
            log::trace!("Not enough data for MACD prediction, returning last known price.");
            return *self.prices.last().unwrap();
        }

        let compute_macd = |short_period, long_period| {
            let short_ema = self.calculate_ema(self.prices.len() - 1, short_period);
            log::trace!("Short EMA: {}", short_ema);

            let long_ema = self.calculate_ema(self.prices.len() - 1, long_period);
            log::trace!("Long EMA: {}", long_ema);

            let macd_line = short_ema - long_ema + 1e-8; // Added small constant to prevent exact zero
            log::trace!("MACD Line: {}", macd_line);

            let macd_lines = self.calculate_macd_lines(short_period, long_period, SIGNAL_PERIOD);
            let signal_line = self.calculate_ema(macd_lines.len() - 1, SIGNAL_PERIOD);
            log::trace!("Signal Line: {}", signal_line);

            let histogram = macd_line - signal_line;
            log::trace!("Histogram: {}", histogram);

            let predicted_price = self.prices[self.prices.len() - 1] + histogram;
            log::trace!("Predicted Price: {}", predicted_price);
            predicted_price
        };

        let predicted_price = compute_macd(self.short_period, self.long_period * 2);

        let last_price = *self.prices.last().unwrap();

        if (predicted_price - last_price).abs() > MACD_THRESHOLD {
            last_price
        } else {
            predicted_price
        }
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

        let slope: f64 = if denominator != 0.0 {
            numerator / denominator
        } else {
            0.0
        };
        let intercept: f64 = y_mean - slope * x_mean;

        return slope * next_timestamp as f64 + intercept;
    }

    pub fn predict_next_price_rsi(&self, period: usize) -> f64 {
        let rsi = self.calculate_rsi(period);
        if rsi > RSI_OVERBOUGHT {
            self.prices.last().unwrap() * 0.99 // assume a 1% price drop
        } else if rsi < RSI_OVERSOLD {
            self.prices.last().unwrap() * 1.01 // assume a 1% price rise
        } else {
            *self.prices.last().unwrap() // no clear signal, return last price
        }
    }

    pub fn predict_next_price_bollinger(&mut self, period: usize) -> f64 {
        self.update_ema(*self.prices.last().unwrap());
        let (lower_band, _, upper_band) = self.calculate_bollinger_bands(period);
        let last_price = *self.prices.last().unwrap();
        if last_price > upper_band {
            (last_price + last_price * 0.99) / 2.0 // take the average of the last price and the price assuming a 1% drop
        } else if last_price < lower_band {
            (last_price + last_price * 1.01) / 2.0 // take the average of the last price and the price assuming a 1% rise
        } else {
            last_price // price is within bands, return last price
        }
    }

    pub fn predict_next_price_fibonacci(&mut self) -> f64 {
        self.update_ema(*self.prices.last().unwrap());
        let (level1, level2, level3, _low) = self.calculate_fibonacci_retracement();
        let last_price = *self.prices.last().unwrap();
        if last_price < level1 {
            last_price * 1.01 // assume a 1% price rise
        } else if last_price < level2 {
            level1 // price might retreat to level1
        } else if last_price < level3 {
            level2 // price might retreat to level2
        } else {
            level3 // price might retreat to level3
        }
    }

    pub fn calculate_std_dev(&self) -> f64 {
        let mean = self.prices.iter().sum::<f64>() / self.prices.len() as f64;
        let variance =
            self.prices.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / self.prices.len() as f64;
        variance.sqrt()
    }

    pub fn is_flash_crash(&self) -> bool {
        if self.last_price < (self.ema_short * self.flash_crash_threshold)
            && self.calculate_rsi(SIGNAL_PERIOD) > RSI_FLASH_CRASH
        {
            return true;
        }
        false
    }

    fn update_ema(&mut self, price: f64) {
        let weight_short = 2.0 / (self.short_period as f64 + 1.0);
        let weight_medium = 2.0 / (self.medium_period as f64 + 1.0);
        let weight_long = 2.0 / (self.long_period as f64 + 1.0);

        if self.prices.len() == 1 {
            self.ema_short = price;
            self.ema_medium = price;
            self.ema_long = price;
        } else {
            self.ema_short = (price - self.ema_short) * weight_short + self.ema_short;
            if self.prices.len() >= self.medium_period {
                self.ema_medium = (price - self.ema_medium) * weight_medium + self.ema_medium;
            }
            if self.prices.len() >= self.long_period {
                self.ema_long = (price - self.ema_long) * weight_long + self.ema_long;
            }
        }
    }

    fn calculate_ema(&self, current_index: usize, period: usize) -> f64 {
        if current_index < period {
            // When there are fewer data points than the period, return SMA
            let sum: f64 = self.prices.iter().take(current_index).sum();
            return sum / current_index as f64;
        }
        // Calculate SMA for the period as the seed of EMA
        let sma: f64 = self
            .prices
            .iter()
            .skip(current_index - period)
            .take(period)
            .sum::<f64>()
            / period as f64;
        let multiplier = 2.0 / (period as f64 + 1.0);

        let ema = self
            .prices
            .iter()
            .enumerate()
            .skip(current_index - period + 1)
            .take(period)
            .fold(sma, |ema, (_i, &price)| (price - ema) * multiplier + ema);

        ema
    }

    fn calculate_macd_lines(&self, short_period: usize, long_period: usize, n: usize) -> Vec<f64> {
        let start_index = self.prices.len().saturating_sub(n);
        let end_index = self.prices.len();
        let mut macd_lines = Vec::new();

        for i in start_index..end_index {
            let short_ema = self.calculate_ema(i, short_period);
            let long_ema = self.calculate_ema(i, long_period);
            let macd_line = short_ema - long_ema + 1e-8; // Add a small constant to prevent exact zero
            macd_lines.push(macd_line);
        }

        macd_lines
    }

    fn calculate_sma(&self, period: usize) -> f64 {
        if self.prices.len() < period {
            return self.prices[self.prices.len() - 1];
        }
        let sum: f64 = self.prices.iter().skip(self.prices.len() - period).sum();
        let sma = sum / period as f64;
        sma
    }

    fn calculate_bollinger_bands(&self, period: usize) -> (f64, f64, f64) {
        let sma = self.calculate_sma(period);
        let std_dev = self.calculate_std_dev();
        let upper_band = sma + (2.0 * std_dev);
        let lower_band = sma - (2.0 * std_dev);
        (upper_band, sma, lower_band)
    }

    fn calculate_rsi(&self, period: usize) -> f64 {
        if self.prices.len() < period + 1 {
            return *self.prices.last().unwrap();
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (self.prices.len() - period)..self.prices.len() {
            let change = self.prices[i] - self.prices[i - 1];
            if change >= 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        let rs = if avg_loss != 0.0 {
            avg_gain / avg_loss
        } else {
            0.0
        };

        100.0 - (100.0 / (1.0 + rs))
    }

    fn calculate_fibonacci_retracement(&self) -> (f64, f64, f64, f64) {
        let high = self
            .prices
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let low = self.prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let diff = high - low;

        let level1 = high - 0.236 * diff;
        let level2 = high - 0.382 * diff;
        let level3 = high - 0.618 * diff;

        (level1, level2, level3, low)
    }

    pub fn majority_vote_predictions(&mut self, period: usize, strategy: TradingStrategy) -> f64 {
        let mut predictions = vec![];

        match strategy {
            TradingStrategy::TrendFollowing => {
                let sma = self.predict_next_price_sma(period);
                let ema = self.predict_next_price_ema(period);
                let regression_prediction =
                    self.predict_next_price_regression(self.timestamps.last().unwrap() + 1);
                predictions.push(sma);
                predictions.push(ema);
                predictions.push(regression_prediction);
                log::trace!(
                    "SMA: {:6.3}, EMA: {:6.3}, REG: {:6.3}",
                    sma,
                    ema,
                    regression_prediction
                );
            }
            TradingStrategy::MeanReversion => {
                let bollinger_prediction = self.predict_next_price_bollinger(period);
                let fibonacci_prediction = self.predict_next_price_fibonacci();
                predictions.push(bollinger_prediction);
                predictions.push(fibonacci_prediction);
                log::trace!(
                    "BOLLINGER: {:6.3}, FIBONACCI: {:6.3}",
                    bollinger_prediction,
                    fibonacci_prediction
                );
            }
            TradingStrategy::Contrarian => {
                let macd = self.predict_next_price_macd();
                let rsi_prediction = self.predict_next_price_rsi(period);
                predictions.push(macd);
                predictions.push(rsi_prediction);
                log::trace!("MACD: {:6.3}, RSI: {:6.3}", macd, rsi_prediction);
            }
        }

        let last_price = self.prices.last().unwrap();

        let mut up_votes = 0;
        let mut down_votes = 0;
        let mut up_sum = 0.0;
        let mut down_sum = 0.0;

        for prediction in predictions {
            if prediction > *last_price {
                up_votes += 1;
                up_sum += prediction;
            } else {
                down_votes += 1;
                down_sum += prediction;
            }
        }

        if up_votes > down_votes {
            if up_votes != 0 {
                up_sum / up_votes as f64
            } else {
                *last_price
            }
        } else if up_votes == down_votes {
            *last_price
        } else {
            if down_votes != 0 {
                down_sum / down_votes as f64
            } else {
                *last_price
            }
        }
    }
}
