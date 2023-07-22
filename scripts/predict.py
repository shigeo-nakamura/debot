#!/usr/bin/env python3

import random_forest as rfp

past_minutes = 180

data = rfp.load_data_from_mongodb(past_minutes)  # Pass 'past_minutes' as an argument
past_data_points = past_minutes * 60  # Convert minutes to seconds
features = rfp.create_features(data, past_data_points)

rfp.train_model(data, features, past_data_points)

for future_minutes in [10, 60, 180]:
    future_time_steps = future_minutes * 60  # Convert minutes to seconds
    last_prediction = rfp.predict(data, features, past_data_points, future_time_steps)
    print(f"For {future_minutes} minutes ahead, the last prediction is: {last_prediction}")
