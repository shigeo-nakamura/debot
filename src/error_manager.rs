use std::time::{Duration, Instant};

pub(crate) struct ErrorManager {
    first_error_time: Option<Instant>,
}

impl ErrorManager {
    pub fn new() -> Self {
        ErrorManager {
            first_error_time: None,
        }
    }

    pub fn save_first_error_time(&mut self) {
        if self.first_error_time.is_none() {
            self.first_error_time = Some(Instant::now());
        }
    }

    pub fn reset_error_time(&mut self) {
        self.first_error_time = None;
    }

    pub fn has_error_duration_passed(&self, error_duration: Duration) -> bool {
        if let Some(first_error_time) = self.first_error_time {
            first_error_time.elapsed() > error_duration
        } else {
            false
        }
    }
}
