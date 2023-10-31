// counter.rs

pub enum CounterType {
    Position,
    Price,
    Balance,
}
pub struct CounterData {
    max: u32,
    counter: std::sync::Mutex<u32>,
}

pub struct Counter {
    position: CounterData,
    price: CounterData,
    balance: CounterData,
}

impl Counter {
    pub fn new(
        max_position_counter: u32,
        max_price_counter: u32,
        max_balance_counter: u32,
        position_counter: u32,
        price_counter: u32,
        balance_counter: u32,
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
            balance: CounterData {
                max: max_balance_counter,
                counter: std::sync::Mutex::new(balance_counter),
            },
        }
    }

    pub fn increment(&self, counter_type: CounterType) -> u32 {
        let counter_data = match counter_type {
            CounterType::Position => &self.position,
            CounterType::Price => &self.price,
            CounterType::Balance => &self.balance,
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
