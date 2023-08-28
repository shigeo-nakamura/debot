// price_history.rs

use std::collections::HashMap;

use crate::utils::ToDateTimeString;
use serde::{Deserialize, Serialize};

use super::{Trend, TrendValue, ValueChange};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum MarketStatus {
    Bull,
    #[default]
    Stay,
    Bear,
    GoldenCross,
    DeadCross,
}

impl MarketStatus {
    pub fn to_int(&self) -> f64 {
        match *self {
            MarketStatus::GoldenCross => 1.0,
            MarketStatus::Bull => 0.75,
            MarketStatus::Stay => 0.5,
            MarketStatus::Bear => 0.25,
            MarketStatus::DeadCross => 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PricePoint {
    pub timestamp: i64,
    pub timestamp_str: String,
    relative_timestamp: Option<i64>,
    pub price: f64,
    pub strength: f64,
}

impl PricePoint {
    pub fn new(price: f64, market_status: MarketStatus, timestamp: Option<i64>) -> Self {
        let time = timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp());
        Self {
            timestamp: time,
            timestamp_str: time.to_datetime_string(),
            relative_timestamp: None,
            price,
            strength: market_status.to_int(),
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
            strength: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PriceHistory {
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
    market_status: MarketStatus,
    prev_market_status: MarketStatus,
    market_status_change_counter: u32,
    atr: HashMap<usize, f64>,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum TradingStrategy {
    TrendFollowing,
}

impl PriceHistory {
    pub fn new(
        short_period: usize,
        medium_period: usize,
        long_period: usize,
        max_size: usize,
        interval: u64,
    ) -> PriceHistory {
        PriceHistory {
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
            market_status: MarketStatus::Stay,
            prev_market_status: MarketStatus::Stay,
            market_status_change_counter: 0,
            atr: HashMap::new(),
        }
    }

    pub fn market_status(&self) -> MarketStatus {
        self.market_status
    }

    pub fn atr(&mut self, period: usize) -> Option<f64> {
        self.atr.get(&period).copied()
    }

    pub fn add_price(&mut self, price: f64, timestamp: Option<i64>) -> PricePoint {
        if self.prices.len() == self.max_size {
            self.prices.remove(0);
        }

        let mut price_point = PricePoint::new(price, self.market_status, timestamp);

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
        self.update_rsi();
        self.update_market_status(price);
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

    pub fn combine_market_status(
        ema_short: &mut TrendValue,
        ema_medium: &mut TrendValue,
        ema_long: &mut TrendValue,
        rsi_short: &mut TrendValue,
        rsi_medium: &mut TrendValue,
        rsi_long: &mut TrendValue,
    ) -> MarketStatus {
        if rsi_short.current == 0.0 || rsi_medium.current == 0.0 || rsi_long.current == 0.0 {
            return MarketStatus::Stay;
        }

        if ema_short.is_up() == Trend::Rise
            && ema_short.current > ema_long.current
            && ema_short.previous().is_some()
            && ema_long.previous().is_some()
        {
            if ema_short.previous().unwrap() <= ema_long.previous().unwrap() {
                return MarketStatus::GoldenCross;
            }
        }
        if ema_short.is_up() == Trend::Fall
            && ema_short.current < ema_long.current
            && ema_short.previous().is_some()
            && ema_long.previous().is_some()
        {
            if ema_short.previous().unwrap() >= ema_long.previous().unwrap() {
                return MarketStatus::DeadCross;
            }
        }

        let bull_conditions = [
            ema_short.is_up() == Trend::Rise,
            ema_medium.is_up() == Trend::Rise,
            ema_long.is_up() == Trend::Rise,
            rsi_short.is_up() == Trend::Rise && rsi_short.current < 40.0,
            rsi_medium.is_up() == Trend::Rise && rsi_medium.current < 40.0,
            rsi_long.is_up() == Trend::Rise && rsi_long.current < 40.0,
        ];

        let bear_conditions = [
            ema_short.is_up() == Trend::Fall,
            ema_medium.is_up() == Trend::Fall,
            ema_long.is_up() == Trend::Fall,
            rsi_short.is_up() == Trend::Fall,
            rsi_medium.is_up() == Trend::Fall,
            rsi_long.is_up() == Trend::Fall,
            rsi_short.is_up() == Trend::Rise && rsi_short.current > 60.0,
            rsi_medium.is_up() == Trend::Rise && rsi_medium.current > 60.0,
            rsi_long.is_up() == Trend::Rise && rsi_long.current > 60.0,
        ];

        let stay_conditions = [
            ema_short.is_up() == Trend::Stay,
            ema_medium.is_up() == Trend::Stay,
            ema_long.is_up() == Trend::Stay,
            rsi_short.is_up() == Trend::Stay,
            rsi_medium.is_up() == Trend::Stay,
            rsi_long.is_up() == Trend::Stay,
            rsi_short.is_up() == Trend::Rise && rsi_short.current > 50.0,
            rsi_medium.is_up() == Trend::Rise && rsi_medium.current > 50.0,
            rsi_long.is_up() == Trend::Rise && rsi_long.current > 50.0,
        ];

        let bull_count = bull_conditions.iter().filter(|&&x| x).count();
        let bear_count = bear_conditions.iter().filter(|&&x| x).count();
        let stay_count = stay_conditions.iter().filter(|&&x| x).count();

        if stay_count >= bull_count && stay_count >= bear_count
            || bear_count + stay_count >= bull_count
            || bull_count == bear_count
        {
            return MarketStatus::Stay;
        } else if bear_count > bull_count {
            return MarketStatus::Bear;
        } else if bull_count - bear_count > 1 {
            return MarketStatus::Bull;
        } else {
            return MarketStatus::Stay;
        }
    }

    fn update_market_status(&mut self, price: f64) {
        self.prev_market_status = self.market_status;

        let new_market_status = Self::combine_market_status(
            &mut self.ema_short,
            &mut self.ema_medium,
            &mut self.ema_long,
            &mut self.rsi_short,
            &mut self.rsi_medium,
            &mut self.rsi_long,
        );

        if new_market_status != self.market_status {
            self.market_status_change_counter += 1;
            if self.market_status_change_counter >= 2 {
                self.market_status = new_market_status;
                self.market_status_change_counter = 0;
            }
        } else {
            self.market_status_change_counter = 0;
        }

        log::info!(
            "{:6.3}({:?}), {:6.2}[{:?}, {:2.1}({:?})] {:6.2}[{:?}, {:2.1}({:?})] {:6.2}[{:?}, {:2.1}({:?})]",
            price,
            self.market_status,
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

    pub fn predict_next_price_sma(&self, period: usize) -> f64 {
        let price = self.prices[self.prices.len() - 1].price;
        let sma = self.calculate_sma(period);
        let diff = sma - price;

        let multiplier = match self.market_status {
            MarketStatus::GoldenCross => 1.02,
            MarketStatus::Bull => 1.015,
            MarketStatus::Stay => 1.0,
            MarketStatus::Bear => 0.99,
            MarketStatus::DeadCross => 0.98,
        };

        price + diff * multiplier
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
                let sma = self.predict_next_price_sma(period);
                predictions.push(sma);
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
