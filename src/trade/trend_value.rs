#[derive(Debug, Clone)]
pub struct TrendValue {
    pub current: f64,
    pub previous_values: Vec<f64>,
    pub period: usize,
    pub threshold: f64,
    pub last_trend: Trend,
    pub consecutive_below_threshold: usize,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub enum Trend {
    Rise,
    Fall,
    #[default]
    Stay,
}

pub trait ValueChange {
    fn is_up(&mut self) -> Trend;
    fn update_value(&mut self, new_val: f64);
    fn previous(&self) -> Option<f64>;
}

impl TrendValue {
    pub fn new(period: usize, threshold: f64) -> Self {
        TrendValue {
            current: 0.0,
            previous_values: vec![],
            period,
            threshold,
            last_trend: Trend::Stay,
            consecutive_below_threshold: 0,
        }
    }
}

impl ValueChange for TrendValue {
    fn is_up(&mut self) -> Trend {
        let current_trend = if let Some(avg) = self.previous() {
            let diff = ((self.current - avg) / avg).abs() * 100.0;
            if diff < self.threshold {
                self.consecutive_below_threshold += 1;
                if self.consecutive_below_threshold >= 10 {
                    Trend::Stay
                } else {
                    self.last_trend.clone()
                }
            } else {
                self.consecutive_below_threshold = 0;
                if self.current > avg {
                    Trend::Rise
                } else {
                    Trend::Fall
                }
            }
        } else {
            Trend::Stay
        };

        self.last_trend = current_trend.clone();
        current_trend
    }

    fn update_value(&mut self, new_val: f64) {
        if self.previous_values.len() >= self.period {
            self.previous_values.remove(0);
        }
        self.previous_values.push(self.current);
        self.current = new_val;
    }

    fn previous(&self) -> Option<f64> {
        if self.previous_values.is_empty() {
            return None;
        }

        let sum: f64 = self.previous_values.iter().sum();
        Some(sum / (self.previous_values.len() as f64))
    }
}
