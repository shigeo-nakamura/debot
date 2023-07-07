#!/usr/bin/env python3

print("hello")

# import pandas as pd
# import numpy as np
# from pymongo import MongoClient
# from sklearn.linear_model import SGDRegressor
# from sklearn.preprocessing import StandardScaler

# def load_data_from_mongodb():
#     # Initialize MongoDB client
#     client = MongoClient('mongodb://localhost:27017/')import pandas as pd
# import numpy as np
# from pymongo import MongoClient
# from sklearn.linear_model import SGDRegressor
# from sklearn.preprocessing import StandardScaler

# def load_data_from_mongodb():
#     # Initialize MongoDB client
#     client = MongoClient('mongodb://localhost:27017/')

#     # Specify the database and collection
#     db = client['your_database_name']
#     collection = db['your_collection_name']

#     # Load data from MongoDB
#     cursor = collection.find({})  # load all documents in the collection
#     data = pd.DataFrame(list(cursor))

#     # Let's say the price data is in fields 'WETH', 'WBTC', 'MATIC' and 'timestamp' of the documents
#     data.set_index('timestamp', inplace=True)  # set timestamp as index
    
#     return data

# def create_features(data, past_data_points):
#     # Create a new DataFrame to hold features
#     features = pd.DataFrame(index=data.index)

#     # Create lag features
#     for i in range(1, past_data_points+1):
#         features[f'WETH_lag_{i}'] = data['WETH'].shift(i)
#         features[f'WBTC_lag_{i}'] = data['WBTC'].shift(i)
#         features[f'MATIC_lag_{i}'] = data['MATIC'].shift(i)

#     # Create moving average feature
#     features['WETH_moving_average'] = data['WETH'].rolling(window=past_data_points).mean()

#     # Create rate of change feature
#     features['WETH_rate_of_change'] = data['WETH'].pct_change()

#     # Remove the first rows which contain NaNs due to lag and moving average features
#     features = features.iloc[past_data_points:]
    
#     return features

# def train_predict(data, features, past_data_points, future_time_steps):
#     # Initialize the SGDRegressor
#     model = SGDRegressor(max_iter=1, learning_rate="constant", eta0=0.01)

#     # StandardScaler for feature normalization
#     scaler = StandardScaler()

#     # Train and update the model for each new data point
#     predictions = []
#     for i in range(past_data_points, len(features)-future_time_steps):
#         X_train = scaler.fit_transform(features.iloc[i-past_data_points:i].values)  # normalize features
#         y_train = data['WETH'].iloc[i-past_data_points:i+future_time_steps].values
#         model.partial_fit(X_train, y_train)
        
#         X_new = scaler.transform(features.iloc[i+future_time_steps:i+future_time_steps+1].values)  # normalize the new feature
#         prediction = model.predict(X_new)  # predict the future price
#         predictions.append(prediction[0])

#     # Print out the predictions
#     print(predictions)

# # Load data from MongoDB
# data = load_data_from_mongodb()

# # Create features
# past_data_points = 10  # number of past data points to consider
# features = create_features(data, past_data_points)

# # Train the model and predict
# future_time_steps = 1  # predict the price 1 step into the future
# train_predict(data, features, past_data_points, future_time_steps)


#     # Specify the database and collection
#     db = client['your_database_name']
#     collection = db['your_collection_name']

#     # Load data from MongoDB
#     cursor = collection.find({})  # load all documents in the collection
#     data = pd.DataFrame(list(cursor))

#     # Let's say the price data is in fields 'WETH', 'WBTC', 'MATIC' and 'timestamp' of the documents
#     data.set_index('timestamp', inplace=True)  # set timestamp as index
    
#     return data

# def create_features(data, past_data_points):
#     # Create a new DataFrame to hold features
#     features = pd.DataFrame(index=data.index)

#     # Create lag features
#     for i in range(1, past_data_points+1):
#         features[f'WETH_lag_{i}'] = data['WETH'].shift(i)
#         features[f'WBTC_lag_{i}'] = data['WBTC'].shift(i)
#         features[f'MATIC_lag_{i}'] = data['MATIC'].shift(i)

#     # Create moving average feature
#     features['WETH_moving_average'] = data['WETH'].rolling(window=past_data_points).mean()

#     # Create rate of change feature
#     features['WETH_rate_of_change'] = data['WETH'].pct_change()

#     # Remove the first rows which contain NaNs due to lag and moving average features
#     features = features.iloc[past_data_points:]
    
#     return features

# def train_predict(data, features, past_data_points, future_time_steps):
#     # Initialize the SGDRegressor
#     model = SGDRegressor(max_iter=1, learning_rate="constant", eta0=0.01)

#     # StandardScaler for feature normalization
#     scaler = StandardScaler()

#     # Train and update the model for each new data point
#     predictions = []
#     for i in range(past_data_points, len(features)-future_time_steps):
#         X_train = scaler.fit_transform(features.iloc[i-past_data_points:i].values)  # normalize features
#         y_train = data['WETH'].iloc[i-past_data_points:i+future_time_steps].values
#         model.partial_fit(X_train, y_train)
        
#         X_new = scaler.transform(features.iloc[i+future_time_steps:i+future_time_steps+1].values)  # normalize the new feature
#         prediction = model.predict(X_new)  # predict the future price
#         predictions.append(prediction[0])

#     # Print out the predictions
#     print(predictions)

# # Load data from MongoDB
# data = load_data_from_mongodb()

# # Create features
# past_data_points = 10  # number of past data points to consider
# features = create_features(data, past_data_points)

# # Train the model and predict
# future_time_steps = 1  # predict the price 1 step into the future
# train_predict(data, features, past_data_points, future_time_steps)
