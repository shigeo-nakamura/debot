#!/usr/bin/env python3

from keras.models import Sequential
from keras.layers import LSTM, Dense
import numpy as np
import pandas as pd
from sklearn.preprocessing import MinMaxScaler
import os
import argparse
import pandas as pd
from pymongo import MongoClient
import datetime
import pytz

# The same load_data_from_mongodb function from your original script
def load_data_from_mongodb(past_minutes):
    mongodb_uri = os.getenv('MONGODB_URI')
    db_name = os.getenv('DB_NAME')
    client = MongoClient(mongodb_uri)
    db = client[db_name]
    collection = db['price']

    current_time = datetime.datetime.now(pytz.utc)
    past_time = current_time - datetime.timedelta(minutes=past_minutes)
    past_time_timestamp = past_time.timestamp()

    cursor = collection.find({
        "trader_name": "BSC-AlgoTrader", 
        "price_point.timestamp": {"$gte": past_time_timestamp}
    }).sort("price_point.timestamp", 1)
    data = pd.DataFrame(list(cursor))
    data['timestamp'] = data['price_point'].apply(lambda x: x['timestamp'])
    data['price'] = data['price_point'].apply(lambda x: x['price'])
    data['timestamp'] = pd.to_datetime(data['timestamp'], unit='s')
    data = data.pivot(index='timestamp', columns='token_name', values='price')
    data = data.resample('1S').interpolate()

    return data

# Prepare the dataset for LSTM
def prepare_dataset(data, look_back, future_minutes):
    # future_minutes into seconds
    future_time_steps = future_minutes * 60
    X, Y = [], []
    for i in range(len(data)-look_back-future_time_steps):
        t = data[i:(i+look_back)]
        X.append(t)
        Y.append(data[i + look_back + future_time_steps])
    return np.array(X), np.array(Y)

# Build the LSTM model
def build_model(look_back):
    model = Sequential()
    model.add(LSTM(4, input_shape=(1, look_back)))
    model.add(Dense(1))
    model.compile(loss='mean_squared_error', optimizer='adam')
    return model

# Scale and split the data
def preprocess_data(data, look_back, future_minutes):
    scaler = MinMaxScaler(feature_range=(0, 1))
    data = scaler.fit_transform(data)
    trainX, trainY = prepare_dataset(data, look_back, future_minutes)
    trainX = np.reshape(trainX, (trainX.shape[0], 1, trainX.shape[1]))
    return scaler, trainX, trainY

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Train and predict cryptocurrency prices')
    parser.add_argument('--mode', type=str, default='train', choices=['train', 'predict'], help='Mode to run the script in')
    parser.add_argument('--past_minutes', type=int, default=180, help='number of past minutes to consider')
    parser.add_argument('--future_minutes', type=int, default=60, help='number of short-term minutes into the future to predict')
    args = parser.parse_args()

    look_back = args.past_minutes * 60  # 60 data points in 1 minutes
    data = load_data_from_mongodb(args.past_minutes)

    if args.mode == 'train':
        scaler, trainX, trainY = preprocess_data(data['WBNB'].values, look_back, args.future_minutes)
        model = build_model(look_back)
        model.fit(trainX, trainY, epochs=100, batch_size=1, verbose=2)
        model.save('lstm_model.h5')

    elif args.mode == 'predict':
        from keras.models import load_model
        model = load_model('lstm_model.h5')
        scaler, trainX, trainY = preprocess_data(data['WBNB'].values, look_back, args.future_minutes)
        prediction = model.predict(np.array([trainX[-1]]))
        prediction = scaler.inverse_transform(prediction)
        print("The prediction is ", prediction[0][0])
