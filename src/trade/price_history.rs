// price_history.rs

use serde::{Deserialize, Serialize};

use crate::utils::ToDateTimeString;

const SIGNAL_PERIOD: usize = 9;
const MACD_THRESHOLD: f64 = 0.1;

// RSI thresholds
const RSI_OVERBOUGHT: f64 = 70.0;
const RSI_OVERSOLD: f64 = 30.0;

// Threshold for detecting flash crash based on RSI
const RSI_FLASH_CRASH: f64 = 85.0;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MarketStatus {
    StrongUp,
    Up,
    WeakUp,
    Down,
    Neutral,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PricePoint {
    pub timestamp: i64,
    pub timestamp_str: String,
    relative_timestamp: Option<i64>,
    pub price: f64,
}

impl PricePoint {
    pub fn new(price: f64, timestamp: Option<i64>) -> Self {
        let time = timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp());
        Self {
            timestamp: time,
            timestamp_str: time.to_datetime_string(),
            relative_timestamp: None,
            price,
        }
    }
}

impl Default for PricePoint {
    fn default() -> Self {
        Self {
            timestamp: 0,
            timestamp_str: String::new(),
            relative_timestamp: None,
            price: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PriceHistory {
    prices: Vec<PricePoint>,
    last_price: f64,
    ema_short: f64,
    ema_medium: f64,
    ema_long: f64,
    short_period: usize,
    medium_period: usize,
    long_period: usize,
    max_size: usize,
    interval: u64,
    flash_crash_threshold: f64,
    first_timestamp: Option<i64>,
    market_status: MarketStatus,
    prev_ema_short: f64,
    prev_ema_medium: f64,
    prev_ema_long: f64,
    // Add the high and low price points over a given period
    high_price: f64,
    low_price: f64,
    // Add the ATR value over a given period
    atr: Option<f64>,
    // ATR period
    atr_period: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum TradingStrategy {
    TrendFollowing,
    MeanReversion,
    Contrarian,
    MLSGDPredictive,
}

impl PriceHistory {
    pub fn new(
        short_period: usize,
        medium_period: usize,
        long_period: usize,
        max_size: usize,
        interval: u64,
        flash_crash_threshold: f64,
    ) -> PriceHistory {
        PriceHistory {
            prices: Vec::with_capacity(max_size),
            last_price: 0.0,
            ema_short: 0.0,
            ema_medium: 0.0,
            ema_long: 0.0,
            short_period,
            medium_period,
            long_period,
            max_size,
            interval,
            flash_crash_threshold,
            first_timestamp: None,
            market_status: MarketStatus::Neutral,
            prev_ema_short: 0.0,
            prev_ema_medium: 0.0,
            prev_ema_long: 0.0,
            high_price: 0.0,
            low_price: 0.0,
            atr: None,
            atr_period: long_period,
        }
    }

    pub fn atr(&self) -> Option<f64> {
        self.atr
    }

    pub fn add_price(&mut self, price: f64, timestamp: Option<i64>) -> PricePoint {
        if self.prices.len() == self.max_size {
            self.prices.remove(0);
        }

        let mut price_point = PricePoint::new(price, timestamp);

        if let Some(prev_point) = self.prices.last() {
            if let Some(relative_time) = price_point.timestamp.checked_sub(prev_point.timestamp) {
                price_point.relative_timestamp = Some(relative_time);
            }
        }

        if self.first_timestamp.is_none() {
            self.first_timestamp = Some(price_point.timestamp);
        }

        self.prices.push(price_point.clone());

        let prev_timestamp = if self.prices.len() >= 2 {
            Some(self.prices[self.prices.len() - 2].timestamp)
        } else {
            None
        };

        self.update_ema(price, price_point.timestamp, prev_timestamp);
        self.update_market_status();
        self.last_price = price;

        // If new price is higher than current high_price, update it
        if price > self.high_price {
            self.high_price = price;
        }

        // If new price is lower than current low_price, update it
        if price < self.low_price {
            self.low_price = price;
        }

        // If the prices vector is larger than the ATR period, calculate the ATR
        if self.prices.len() > self.atr_period {
            self.calculate_atr();
        }

        price_point
    }

    fn calculate_atr(&mut self) {
        if self.prices.len() < self.atr_period + 1 {
            return; // Not enough data to calculate ATR
        }

        let atr_prices = &self.prices[self.prices.len() - self.atr_period..];

        let current_high = atr_prices
            .iter()
            .map(|p| p.price)
            .fold(f64::NEG_INFINITY, f64::max);
        let current_low = atr_prices
            .iter()
            .map(|p| p.price)
            .fold(f64::INFINITY, f64::min);
        let previous_close = self.prices[self.prices.len() - self.atr_period - 1].price;

        let tr = [
            current_high - current_low,
            (current_high - previous_close).abs(),
            (current_low - previous_close).abs(),
        ]
        .iter()
        .cloned()
        .fold(f64::NAN, f64::max);

        if self.atr == None {
            // This is the first time we calculate ATR, so we calculate the average TR over atr_period
            let tr_sum: f64 = atr_prices
                .windows(2)
                .map(|window| {
                    let high = window
                        .iter()
                        .map(|pp| pp.price)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let low = window
                        .iter()
                        .map(|pp| pp.price)
                        .fold(f64::INFINITY, f64::min);
                    let previous_close = window[0].price;
                    [
                        high - low,
                        (high - previous_close).abs(),
                        (low - previous_close).abs(),
                    ]
                    .iter()
                    .cloned()
                    .fold(f64::NAN, f64::max)
                })
                .sum();
            self.atr = Some(tr_sum / self.atr_period as f64);
        } else {
            // We update ATR using the formula
            self.atr = Some(
                (self.atr.unwrap() * (self.atr_period as f64 - 1.0) + tr) / self.atr_period as f64,
            );
        }
    }

    fn update_market_status(&mut self) {
        if self.ema_short > self.ema_medium && self.ema_medium > self.ema_long {
            if self.ema_short > self.prev_ema_short && self.ema_medium > self.prev_ema_medium {
                self.market_status = MarketStatus::StrongUp;
            } else {
                self.market_status = MarketStatus::Up;
            }
        } else if self.ema_short > self.ema_medium && self.ema_medium <= self.ema_long {
            if self.ema_short < self.prev_ema_short {
                self.market_status = MarketStatus::Down;
            } else {
                self.market_status = MarketStatus::WeakUp;
            }
        } else if self.ema_short > self.ema_long && self.ema_short <= self.ema_medium {
            self.market_status = MarketStatus::Up;
        } else if self.ema_short < self.ema_medium {
            self.market_status = MarketStatus::Down;
        } else {
            self.market_status = MarketStatus::Neutral;
        }

        // update previous EMAs
        self.prev_ema_short = self.ema_short;
        self.prev_ema_medium = self.ema_medium;
        self.prev_ema_long = self.ema_long;
    }

    pub fn predict_next_price_ema(&self, period: usize) -> f64 {
        let predict = |len, ema| {
            if self.prices.len() >= len {
                2.0 * ema - self.prices[self.prices.len() - 1].price
            } else {
                self.prices[self.prices.len() - 1].price
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
            return self.prices.last().unwrap().price;
        }

        let compute_macd = |short_period, long_period| {
            let short_ema = self.calculate_ema(self.prices.len() - 1, short_period);
            let long_ema = self.calculate_ema(self.prices.len() - 1, long_period);
            let macd_line = short_ema - long_ema + 1e-8;
            let macd_lines = self.calculate_macd_lines(short_period, long_period, SIGNAL_PERIOD);
            let signal_line = self.calculate_ema(macd_lines.len() - 1, SIGNAL_PERIOD);
            let histogram = macd_line - signal_line;
            let predicted_price = self.prices[self.prices.len() - 1].price + histogram;

            log::trace!("Short EMA: {}, Long EMA: {}, MACD Line: {}, Signal Line: {}, Histogram: {}, Predicted Price: {}", 
                short_ema, long_ema, macd_line, signal_line, histogram, predicted_price);

            predicted_price
        };

        let predicted_price = compute_macd(self.short_period, self.long_period * 2);

        let last_price = self.prices.last().unwrap().price;

        if (predicted_price - last_price).abs() > MACD_THRESHOLD {
            last_price
        } else {
            predicted_price
        }
    }

    fn predict_next_price_regression(&self, next_relative_timestamp: i64, period: usize) -> f64 {
        if self.first_timestamp.is_none() || self.prices.len() < period {
            return self.prices.last().unwrap().price;
        }

        let recent_prices = &self.prices[self.prices.len() - period..];

        let x_mean: f64 = recent_prices
            .iter()
            .filter_map(|p| p.relative_timestamp) // Only consider PricePoints with a defined relative_timestamp
            .sum::<i64>() as f64
            / period as f64;
        let y_mean: f64 = recent_prices.iter().map(|p| p.price).sum::<f64>() / period as f64;

        let mut numerator: f64 = 0.0;
        let mut denominator: f64 = 0.0;

        for price_point in recent_prices {
            if let Some(relative_timestamp) = price_point.relative_timestamp {
                let x = relative_timestamp as f64 - x_mean;
                numerator += x * (price_point.price - y_mean);
                denominator += x.powi(2);
            }
        }

        let slope: f64 = if denominator != 0.0 {
            numerator / denominator
        } else {
            0.0
        };
        let intercept: f64 = y_mean - slope * x_mean;

        return slope
            * (next_relative_timestamp as f64
                + recent_prices
                    .last()
                    .unwrap()
                    .relative_timestamp
                    .unwrap_or(0) as f64)
            + intercept;
    }

    pub fn predict_next_price_rsi(&self, period: usize) -> f64 {
        let rsi = self.calculate_rsi(period);
        if rsi > RSI_OVERBOUGHT {
            self.prices.last().unwrap().price * 0.99 // assume a 1% price drop
        } else if rsi < RSI_OVERSOLD {
            self.prices.last().unwrap().price * 1.01 // assume a 1% price rise
        } else {
            self.prices.last().unwrap().price // no clear signal, return last price
        }
    }

    pub fn predict_next_price_bollinger(&mut self, period: usize) -> f64 {
        let (lower_band, _, upper_band) = self.calculate_bollinger_bands(period);
        let last_price = self.prices.last().unwrap().price;
        if last_price > upper_band {
            (last_price + last_price * 0.99) / 2.0 // take the average of the last price and the price assuming a 1% drop
        } else if last_price < lower_band {
            (last_price + last_price * 1.01) / 2.0 // take the average of the last price and the price assuming a 1% rise
        } else {
            last_price // price is within bands, return last price
        }
    }

    pub fn predict_next_price_fibonacci(&mut self) -> f64 {
        let (level1, level2, level3, _low) = self.calculate_fibonacci_retracement();
        let last_price = self.prices.last().unwrap().price;
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

    pub fn predict_next_price_sdg(&self, _period: usize) -> f64 {
        self.prices.last().unwrap().price
    }

    pub fn calculate_std_dev(&self) -> f64 {
        let mean = self.prices.iter().map(|p| p.price).sum::<f64>() / self.prices.len() as f64;
        let variance = self
            .prices
            .iter()
            .map(|p| (p.price - mean).powi(2))
            .sum::<f64>()
            / self.prices.len() as f64;
        variance.sqrt()
    }

    #[allow(dead_code)]
    pub fn is_flash_crash(&self) -> bool {
        if self.last_price < (self.ema_short * self.flash_crash_threshold)
            && self.calculate_rsi(SIGNAL_PERIOD) > RSI_FLASH_CRASH
        {
            return true;
        }
        false
    }

    fn update_ema(&mut self, price: f64, timestamp: i64, prev_timestamp: Option<i64>) {
        let weight_short = 2.0 / (self.short_period as f64 + 1.0);
        let weight_medium = 2.0 / (self.medium_period as f64 + 1.0);
        let weight_long = 2.0 / (self.long_period as f64 + 1.0);

        let time_difference = match prev_timestamp {
            Some(prev_timestamp) => (timestamp - prev_timestamp) as f64 / self.interval as f64,
            None => 1.0,
        };

        if self.ema_short == 0.0 {
            self.ema_short = price;
        } else {
            self.ema_short =
                (price - self.ema_short) * weight_short * time_difference + self.ema_short;
        }

        if self.ema_medium == 0.0 {
            self.ema_medium = price;
        } else if self.prices.len() >= self.medium_period {
            self.ema_medium =
                (price - self.ema_medium) * weight_medium * time_difference + self.ema_medium;
        }

        if self.ema_long == 0.0 {
            self.ema_long = price;
        } else if self.prices.len() >= self.long_period {
            self.ema_long = (price - self.ema_long) * weight_long * time_difference + self.ema_long;
        }
    }

    fn calculate_ema(&self, current_index: usize, period: usize) -> f64 {
        if current_index < period {
            // When there are fewer data points than the period, return SMA
            let sum: f64 = self
                .prices
                .iter()
                .take(current_index)
                .map(|p| p.price)
                .sum();
            return sum / current_index as f64;
        }
        let sma: f64 = self.calculate_sma(period);
        let multiplier = 2.0 / (period as f64 + 1.0);
        let ema = self
            .prices
            .iter()
            .enumerate()
            .skip(current_index - period + 1)
            .take(period)
            .fold(sma, |ema, (_i, price)| {
                (price.price - ema) * multiplier + ema
            });

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
            return self.prices[self.prices.len() - 1].price;
        }
        let sum: f64 = self
            .prices
            .iter()
            .skip(self.prices.len() - period)
            .map(|p| p.price)
            .sum();
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
            return self.prices.last().unwrap().price;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (self.prices.len() - period)..self.prices.len() {
            let change = self.prices[i].price - self.prices[i - 1].price;
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
            .map(|p| p.price)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = self
            .prices
            .iter()
            .map(|p| p.price)
            .fold(f64::INFINITY, f64::min);
        let diff = high - low;

        let level1 = high - 0.236 * diff;
        let level2 = high - 0.382 * diff;
        let level3 = high - 0.618 * diff;

        (level1, level2, level3, low)
    }

    pub fn majority_vote_predictions(
        &mut self,
        period: usize,
        prediction_interval_secs: u64,
        strategy: TradingStrategy,
    ) -> f64 {
        let mut predictions = vec![];

        match strategy {
            TradingStrategy::TrendFollowing => {
                let sma = self.predict_next_price_sma(period);
                let ema = self.predict_next_price_ema(period);
                let regression_prediction =
                    self.predict_next_price_regression(prediction_interval_secs as i64, period);
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
            TradingStrategy::MLSGDPredictive => {
                let sdg = self.predict_next_price_sdg(period);
                predictions.push(sdg);
            }
        }

        let last_price = self.prices.last().unwrap().price;

        let mut up_votes = 0;
        let mut down_votes = 0;
        let mut up_sum = 0.0;
        let mut down_sum = 0.0;

        for prediction in predictions {
            if prediction > last_price {
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
                last_price
            }
        } else if up_votes == down_votes {
            last_price
        } else {
            if down_votes != 0 {
                down_sum / down_votes as f64
            } else {
                last_price
            }
        }
    }
}
