pub struct ErrorManager {
    error_count: u32,
}

impl ErrorManager {
    pub fn new() -> Self {
        ErrorManager { error_count: 0 }
    }

    pub fn increment_error_count(&mut self) {
        self.error_count += 1;
    }

    pub fn get_error_count(&self) -> u32 {
        self.error_count
    }

    pub fn reset_error_count(&mut self) {
        self.error_count = 0;
    }
}
