// price_history.rs

use std::collections::HashMap;

use crate::utils::ToDateTimeString;
use serde::{Deserialize, Serialize};

use super::{Trend, TrendValue, ValueChange};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PricePoint {
    pub timestamp: i64,
    pub timestamp_str: String,
    relative_timestamp: Option<i64>,
    pub price: f64,
    pub sentiment: f64,
    pub sentiment_moving_avg: f64,
}

impl PricePoint {
    pub fn new(
        price: f64,
        sentiment: f64,
        sentiment_moving_avg: f64,
        timestamp: Option<i64>,
    ) -> Self {
        let time = timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp());
        Self {
            timestamp: time,
            timestamp_str: time.to_datetime_string(),
            relative_timestamp: None,
            price,
            sentiment,
            sentiment_moving_avg,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PriceHistory {
    name: String,
    prices: Vec<PricePoint>,
    last_price: f64,
    ema_short: TrendValue,
    ema_medium: TrendValue,
    ema_long: TrendValue,
    rsi_short: TrendValue,
    rsi_medium: TrendValue,
    rsi_long: TrendValue,
    short_period: usize,
    medium_period: usize,
    long_period: usize,
    max_size: usize,
    interval: u64,
    first_timestamp: Option<i64>,
    sentiment: f64,
    sentiment_moving_avg: f64,
    atr: HashMap<usize, f64>,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum TradingStrategy {
    TrendFollowing,
}

impl PriceHistory {
    pub fn new(
        name: String,
        short_period: usize,
        medium_period: usize,
        long_period: usize,
        max_size: usize,
        interval: u64,
    ) -> PriceHistory {
        PriceHistory {
            name,
            prices: Vec::with_capacity(max_size),
            last_price: 0.0,
            ema_short: TrendValue::new(100, 0.004),
            ema_medium: TrendValue::new(100, 0.002),
            ema_long: TrendValue::new(100, 0.001),
            rsi_short: TrendValue::new(10, 4.0),
            rsi_medium: TrendValue::new(10, 2.0),
            rsi_long: TrendValue::new(10, 1.0),
            short_period,
            medium_period,
            long_period,
            max_size,
            interval,
            first_timestamp: None,
            sentiment: 0.0,
            sentiment_moving_avg: 0.0,
            atr: HashMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn atr(&mut self, period: usize) -> Option<f64> {
        self.atr.get(&period).copied()
    }

    pub fn sentiment(&self) -> f64 {
        self.sentiment
    }

    pub fn add_price(&mut self, price: f64, timestamp: Option<i64>) -> PricePoint {
        if self.prices.len() == self.max_size {
            self.prices.remove(0);
        }

        let mut price_point =
            PricePoint::new(price, self.sentiment, self.sentiment_moving_avg, timestamp);

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

        let alpha = 0.002;
        self.sentiment_moving_avg =
            (1.0 - alpha) * self.sentiment_moving_avg + alpha * self.sentiment;

        self.update_ema(price, price_point.timestamp, prev_timestamp);
        self.update_rsi();
        self.update_market_sentiment(price);
        self.last_price = price;

        price_point
    }

    pub fn update_atr(&mut self, period: usize) {
        if self.prices.len() < period {
            return;
        }

        let window_size = period;
        let atr_prices = &self.prices;

        let current_high = atr_prices
            .iter()
            .map(|p| p.price)
            .fold(f64::NEG_INFINITY, f64::max);

        let current_low = atr_prices
            .iter()
            .map(|p| p.price)
            .fold(f64::INFINITY, f64::min);

        let tr = current_high - current_low;

        let atr = self.atr.get(&period);
        let mut tr_sum = 0.0;

        if atr.is_none() {
            for window in atr_prices.windows(window_size) {
                let high = window
                    .iter()
                    .map(|pp| pp.price)
                    .fold(f64::NEG_INFINITY, f64::max);
                let low = window
                    .iter()
                    .map(|pp| pp.price)
                    .fold(f64::INFINITY, f64::min);
                let window_tr = high - low;
                tr_sum += window_tr;
            }
            let new_atr = tr_sum / atr_prices.windows(window_size).len() as f64;
            self.atr.insert(period, new_atr);
        } else {
            let new_atr = (atr.unwrap() * (period as f64 - 1.0) + tr) / period as f64;
            self.atr.insert(period, new_atr);
        }
    }

    fn update_rsi(&mut self) {
        let new_rsi_short = self.calculate_rsi(self.short_period);
        if new_rsi_short > 0.0 {
            self.rsi_short.update_value(new_rsi_short);
        }

        let new_rsi_medium = self.calculate_rsi(self.medium_period);
        if new_rsi_medium > 0.0 {
            self.rsi_medium.update_value(new_rsi_medium);
        }

        let new_rsi_long = self.calculate_rsi(self.long_period);
        if new_rsi_long > 0.0 {
            self.rsi_long.update_value(new_rsi_long);
        }
    }

    pub fn combine_market_sentiment(
        ema_short: &mut TrendValue,
        ema_medium: &mut TrendValue,
        ema_long: &mut TrendValue,
        rsi_short: &mut TrendValue,
        rsi_medium: &mut TrendValue,
        rsi_long: &mut TrendValue,
    ) -> f64 {
        if rsi_short.current == 0.0 || rsi_medium.current == 0.0 || rsi_long.current == 0.0 {
            return 0.0;
        }

        let bull_conditions = [
            ema_short.is_up() == Trend::Rise,
            ema_medium.is_up() == Trend::Rise,
            ema_long.is_up() == Trend::Rise,
            rsi_short.is_up() == Trend::Rise,
            rsi_medium.is_up() == Trend::Rise,
            rsi_long.is_up() == Trend::Rise,
        ];

        let bear_conditions = [
            ema_short.is_up() == Trend::Fall,
            ema_medium.is_up() == Trend::Fall,
            ema_long.is_up() == Trend::Fall,
            rsi_short.is_up() == Trend::Fall,
            rsi_medium.is_up() == Trend::Fall,
            rsi_long.is_up() == Trend::Fall,
        ];

        let bull_ratio =
            bull_conditions.iter().filter(|&&x| x).count() as f64 / bull_conditions.len() as f64;
        let bear_ratio =
            bear_conditions.iter().filter(|&&x| x).count() as f64 / bear_conditions.len() as f64;

        0.5 * (bull_ratio - bear_ratio)
    }

    fn update_market_sentiment(&mut self, price: f64) {
        let new_sentiment = Self::combine_market_sentiment(
            &mut self.ema_short,
            &mut self.ema_medium,
            &mut self.ema_long,
            &mut self.rsi_short,
            &mut self.rsi_medium,
            &mut self.rsi_long,
        );

        let alpha = 0.1;
        let smoothed_sentiment = (1.0 - alpha) * self.sentiment + alpha * new_sentiment;
        self.sentiment = smoothed_sentiment;

        log::debug!(
            "{}:{:6.2}({:0.2}) {:6.2}[{:?},{:2.1}({:?})] {:6.2}[{:?},{:2.1}({:?})] {:6.2}[{:?},{:2.1}({:?})]",
            self.name,
            price,
            self.sentiment,
            self.ema_short.current.clone(),
            self.ema_short.is_up(),
            self.rsi_short.current.clone(),
            self.rsi_short.is_up(),
            self.ema_medium.current.clone(),
            self.ema_medium.is_up(),
            self.rsi_medium.current.clone(),
            self.rsi_medium.is_up(),
            self.ema_long.current.clone(),
            self.ema_long.is_up(),
            self.rsi_long.current.clone(),
            self.rsi_long.is_up(),
        );
    }

    pub fn calculate_sentiment_stats(&self, mut period: usize) -> Option<(f64, f64)> {
        let n = self.prices.len();
        const SECONDS_IN_A_DAY: u32 = 60 * 60 * 24;
        let max_period: usize = (SECONDS_IN_A_DAY / self.interval as u32)
            .try_into()
            .unwrap();
        if period > max_period {
            period = max_period;
        }
        if n < period {
            return None; // Not enough data points
        }

        // Slice the last `period` prices
        let last_prices = &self.prices[n - period..];

        // Calculate average
        let sum: f64 = last_prices.iter().map(|x| x.sentiment_moving_avg).sum();
        let avg = sum / (period as f64);

        // Calculate standard deviation
        let sum_of_squares: f64 = last_prices
            .iter()
            .map(|x| (x.sentiment_moving_avg - avg).powi(2))
            .sum();
        let std_dev = (sum_of_squares / (period as f64)).sqrt();

        log::trace!("avg = {}, std_dev = {}", avg, std_dev);

        Some((avg, std_dev))
    }

    pub fn predict_next_price_ema(&self, period: usize) -> f64 {
        let price = self.prices[self.prices.len() - 1].price;
        let ema = self.calculate_ema(period);
        let diff = ema - price;

        let prescaler =
            if let Some((avg, std_dev)) = self.calculate_sentiment_stats(self.medium_period) {
                match self.sentiment_moving_avg {
                    x if x < avg - std_dev * 2.0 => 3.0,
                    x if x < avg - std_dev * 1.5 => 2.0,
                    x if x < avg - std_dev * 1.2 => 1.5,
                    _ => 1.0,
                }
            } else {
                1.0
            };

        return ema + diff * prescaler;
    }

    fn update_ema(&mut self, price: f64, timestamp: i64, prev_timestamp: Option<i64>) {
        let time_difference =
            prev_timestamp.map_or(1.0, |prev| (timestamp - prev) as f64 / self.interval as f64);

        let update_one_ema = |ema: &mut TrendValue, period: usize| {
            let weight = 2.0 / (period as f64 + 1.0);
            let prev_ema = ema.current;
            ema.update_value(if ema.current == 0.0 {
                price
            } else if self.prices.len() >= period {
                (price - prev_ema) * weight * time_difference + prev_ema
            } else {
                ema.current
            });
        };

        update_one_ema(&mut self.ema_short, self.short_period);
        update_one_ema(&mut self.ema_medium, self.medium_period);
        update_one_ema(&mut self.ema_long, self.long_period);
    }

    fn calculate_sma(&self, period: usize) -> f64 {
        let price = self.prices[self.prices.len() - 1].price;
        if self.prices.len() < period {
            return price;
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

    fn calculate_ema(&self, period: usize) -> f64 {
        if self.prices.len() < period {
            return self.prices[self.prices.len() - 1].price;
        }

        let mut ema = self.calculate_sma(period);

        let multiplier = 2.0 / (period as f64 + 1.0);

        for p in self.prices.iter().skip(self.prices.len() - period + 1) {
            ema = (p.price - ema) * multiplier + ema;
        }

        ema
    }

    fn calculate_rsi(&self, period: usize) -> f64 {
        if self.prices.len() < period + 1 {
            return 0.0;
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

    pub fn majority_vote_predictions(&mut self, period: usize, strategy: TradingStrategy) -> f64 {
        let mut predictions = vec![];

        match strategy {
            TradingStrategy::TrendFollowing => {
                let ema = self.predict_next_price_ema(period);
                predictions.push(ema);
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
