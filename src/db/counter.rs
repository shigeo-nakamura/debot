// counter.rs

pub enum CounterType {
    Position,
    Price,
    Performance,
}
pub struct CounterData {
    max: u32,
    counter: std::sync::Mutex<u32>,
}

pub struct Counter {
    position: CounterData,
    price: CounterData,
    performance: CounterData,
}

impl Counter {
    pub fn new(
        max_position_counter: u32,
        max_price_counter: u32,
        max_performance_counter: u32,
        position_counter: u32,
        price_counter: u32,
        performance_counter: u32,
    ) -> Self {
        Self {
            position: CounterData {
                max: max_position_counter,
                counter: std::sync::Mutex::new(position_counter),
            },
            price: CounterData {
                max: max_price_counter,
                counter: std::sync::Mutex::new(price_counter),
            },
            performance: CounterData {
                max: max_performance_counter,
                counter: std::sync::Mutex::new(performance_counter),
            },
        }
    }

    pub fn increment(&self, counter_type: CounterType) -> u32 {
        let counter_data = match counter_type {
            CounterType::Position => &self.position,
            CounterType::Price => &self.price,
            CounterType::Performance => &self.performance,
        };

        let mut counter = counter_data.counter.lock().unwrap();
        *counter += 1;
        let mut id = *counter % (counter_data.max + 1);
        if id == 0 {
            id = 1;
        }
        *counter = id;
        drop(counter);
        id
    }
}
