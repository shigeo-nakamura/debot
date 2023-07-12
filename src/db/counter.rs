// counter.rs

pub enum CounterType {
    Position,
    Price,
    Performance,
}
pub struct Counter {
    max_counter: u32,
    position_counter: std::sync::Mutex<u32>,
    price_counter: std::sync::Mutex<u32>,
    performance_counter: std::sync::Mutex<u32>,
}

impl Counter {
    pub fn new(
        max_counter: u32,
        position_counter: u32,
        price_counter: u32,
        performance_counter: u32,
    ) -> Self {
        Self {
            max_counter,
            position_counter: std::sync::Mutex::new(position_counter),
            price_counter: std::sync::Mutex::new(price_counter),
            performance_counter: std::sync::Mutex::new(performance_counter),
        }
    }

    pub fn increment(&self, counter_type: CounterType) -> u32 {
        let counter = match counter_type {
            CounterType::Position => &self.position_counter,
            CounterType::Price => &self.price_counter,
            CounterType::Performance => &self.performance_counter,
        };

        let mut counter = counter.lock().unwrap();
        *counter += 1;
        let mut id = *counter % (self.max_counter + 1);
        if id == 0 {
            id = 1;
        }
        *counter = id;
        drop(counter);
        id
    }
}
