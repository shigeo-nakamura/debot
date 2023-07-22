#!/usr/bin/env python3

from keras.models import Sequential
from keras.layers import LSTM, Dense
import numpy as np
import pandas as pd
from sklearn.preprocessing import MinMaxScaler
import os
import argparse
from pymongo import MongoClient
import datetime
import pytz
from sklearn.metrics import mean_squared_error

def calculate_SMA(data, window=10):
    return data.rolling(window).mean()

def calculate_RSI(data, window=14):
    delta = data.diff()
    up_days = delta.copy()
    up_days[delta <= 0] = 0.0
    down_days = abs(delta.copy())
    down_days[delta > 0] = 0.0
    RS_up = up_days.rolling(window).mean()
    RS_down = down_days.rolling(window).mean()
    return 100 - 100 / (1 + RS_up / RS_down)

def calculate_MACD(data, short_window=12, long_window=26):
    ShortEMA = data.ewm(span=short_window, adjust=False).mean()
    LongEMA = data.ewm(span=long_window, adjust=False).mean()
    MACD = ShortEMA - LongEMA
    signal = MACD.ewm(span=9, adjust=False).mean()
    return MACD, signal

def load_data_from_mongodb(past_minutes):
    print("Loading data from MongoDB...")  # Add this line
    mongodb_uri = os.getenv('MONGODB_URI')
    db_name = os.getenv('DB_NAME')
    client = MongoClient(mongodb_uri)
    db = client[db_name]
    collection = db['price']

    current_time = datetime.datetime.now(pytz.utc)
    past_time = current_time - datetime.timedelta(minutes=past_minutes)
    past_time_timestamp = past_time.timestamp()

    print(f"Querying data from past {past_minutes} minutes...")  # Add this line
    cursor = collection.find({
        "trader_name": "BSC-AlgoTrader", 
        "price_point.timestamp": {"$gte": past_time_timestamp}
    }).sort("price_point.timestamp", 1)

    print("Converting queried data to DataFrame...")  # Add this line
    data = pd.DataFrame(list(cursor))
    data['timestamp'] = data['price_point'].apply(lambda x: x['timestamp'])
    data['price'] = data['price_point'].apply(lambda x: x['price'])
    data['timestamp'] = pd.to_datetime(data['timestamp'], unit='s')
    data = data.pivot(index='timestamp', columns='token_name', values='price')
    data = data.resample('1S').interpolate()

    # Calculate moving average, RSI, and MACD for each token
    for token in data.columns:
        data[token+'_SMA'] = calculate_SMA(data[token])
        data[token+'_RSI'] = calculate_RSI(data[token])
        data[token+'_MACD'], data[token+'_Signal'] = calculate_MACD(data[token])

    print("Data loaded successfully.")  # Add this line
    return data


def preprocess_data(data, look_back, future_minutes, token_name):
    print("Preprocessing data...")  # Add this line

    if data.empty:
        print("The input data frame is empty. Please check if the data is loaded correctly.")
        raise ValueError("The input data frame is empty. Please check if the data is loaded correctly.")

    print("Scaling data...")  # Add this line
    scaler = MinMaxScaler(feature_range=(0, 1))
    data_scaled = pd.DataFrame(scaler.fit_transform(data.values), columns=data.columns, index=data.index)

    print("Preparing dataset...")  # Add this line
    X, Y = prepare_dataset(data_scaled, look_back, future_minutes, token_name)

    train_size = int(len(X) * 0.8)
    trainX, testX = X[0:train_size], X[train_size:]
    trainY, testY = Y[0:train_size], Y[train_size:]

    print(f"Shape of trainX: {trainX.shape}")
    print(f"Shape of trainY: {trainY.shape}")
    print(f"Shape of testX: {testX.shape}")
    print(f"Shape of testY: {testY.shape}")

    print("Data preprocessed successfully.")  # Add this line
    return scaler, trainX, trainY, testX, testY

def prepare_dataset(data, look_back, future_minutes, token_name):
    future_time_steps = future_minutes * 6
    X, Y = [], []
    for i in range(len(data)-look_back-future_time_steps):
        t = data[i:(i+look_back)].values  # Fetch values of all features
        X.append(t)
        Y.append(data.iloc[i + look_back + future_time_steps][token_name])  # Replace 'token_name' with your token of interest
    return np.array(X), np.array(Y)

def build_model(timesteps, num_features):
    model = Sequential()
    model.add(LSTM(4, input_shape=(timesteps, num_features)))
    model.add(Dense(1))
    model.compile(loss='mean_squared_error', optimizer='adam')
    return model
    return model

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Train and predict cryptocurrency prices')
    parser.add_argument('--mode', type=str, default='train', choices=['train', 'predict', 'test'], help='Mode to run the script in')
    parser.add_argument('--past_minutes', type=int, default=180, help='number of past minutes to consider')
    parser.add_argument('--future_minutes', type=int, default=60, help='number of short-term minutes into the future to predict')
    parser.add_argument('--token_name', type=str, default='ETH', help='token to predict')
    args = parser.parse_args()

    look_back = args.past_minutes * 6
    data = load_data_from_mongodb(args.past_minutes)

    if args.mode == 'train':
        scaler, trainX, trainY = preprocess_data(data, look_back, args.future_minutes, args.token_name)
        
        # print shape of trainX and trainY
        print(f"Shape of trainX: {trainX.shape}")
        print(f"Shape of trainY: {trainY.shape}")
        print(f"First few elements of trainX: {trainX[:2]}")
        print(f"First few elements of trainY: {trainY[:2]}")

        num_features = trainX.shape[2]  # update num_features based on trainX's shape
        model = build_model(look_back, num_features)
        model.fit(trainX, trainY, epochs=100, batch_size=1, verbose=1)
        model.save('lstm_model.h5')

    elif args.mode == 'predict':
        from keras.models import load_model
        model = load_model('lstm_model.h5')
        scaler, trainX, trainY = preprocess_data(data, look_back, args.future_minutes, args.token_name)
        prediction = model.predict(np.array([trainX[-1]]))
        prediction = scaler.inverse_transform(prediction)
        print("The prediction is ", prediction[0][0])

    elif args.mode == 'test':
        from keras.models import load_model
        model = load_model('lstm_model.h5')
        scaler, _, _, testX, testY = preprocess_data(data, look_back, args.future_minutes, args.token_name)
        test_prediction = model.predict(testX)
        test_prediction = scaler.inverse_transform(test_prediction)
        testY = scaler.inverse_transform([testY])  # scale back the testY data
        mse = mean_squared_error(testY[0], test_prediction)
        print("The MSE on the test set is ", mse)