#!/usr/bin/env python3

import lstm

past_minutes = 120

data = lstm.load_data_from_mongodb(past_minutes)  # Pass 'past_minutes' as an argument
look_back = past_minutes * 60  # 60 data points in 1 minutes

lstm.train_model(data['WBNB'].values, look_back)

for future_minutes in [10, 60, 120]:
    last_prediction = lstm.predict(data['WBNB'].values, look_back, future_minutes)
    print(f"For {future_minutes} minutes ahead, the last prediction is: {last_prediction}")
