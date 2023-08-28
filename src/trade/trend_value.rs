#[derive(Debug, Clone)]
pub struct TrendValue {
    pub current: f64,
    pub previous_values: Vec<f64>,
    pub period: usize,
    pub threshold: f64,
    pub last_trend: Trend,
    pub consecutive_below_threshold: usize,
    normalize: bool,
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
    pub fn new(period: usize, threshold: f64, normalize: bool) -> Self {
        TrendValue {
            current: 0.0,
            previous_values: vec![],
            period,
            threshold,
            last_trend: Trend::Stay,
            consecutive_below_threshold: 0,
            normalize,
        }
    }

    fn slope(&self) -> Option<f64> {
        if self.previous_values.len() < self.period {
            return None;
        }
        let sum_y: f64 = self.previous_values.iter().sum();
        let mean_y = sum_y / (self.period as f64);

        let sum_x: f64 = (0..self.period).map(|x| x as f64).sum();
        let mean_x = sum_x / (self.period as f64);

        let numerator: f64 = (0..self.period)
            .map(|i| (i as f64 - mean_x) * (self.previous_values[i] - mean_y))
            .sum();
        let denominator: f64 = (0..self.period).map(|i| (i as f64 - mean_x).powi(2)).sum();
        if denominator == 0.0 {
            return None;
        }

        Some(numerator / denominator)
    }
}

impl ValueChange for TrendValue {
    fn is_up(&mut self) -> Trend {
        if let Some(slope) = self.slope() {
            // Normalize the slope
            let normalized_slope = match self.normalize {
                true => slope / self.current.abs(),
                false => slope,
            };

            // Check against the threshold
            if normalized_slope.abs() >= self.threshold {
                self.consecutive_below_threshold = 0;

                if normalized_slope > 0.0 {
                    self.last_trend = Trend::Rise;
                    return Trend::Rise;
                } else {
                    self.last_trend = Trend::Fall;
                    return Trend::Fall;
                }
            } else {
                self.consecutive_below_threshold += 1;
                if self.consecutive_below_threshold >= 10 {
                    return Trend::Stay;
                } else {
                    return self.last_trend.clone();
                }
            }
        }

        Trend::Stay
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
