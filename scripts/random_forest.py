#!/usr/bin/env python3

import os
import argparse
import pandas as pd
import numpy as np
from pymongo import MongoClient
from sklearn.ensemble import RandomForestRegressor
from sklearn.preprocessing import StandardScaler
import gridfs
import pickle

def load_data_from_mongodb():
    mongodb_uri = os.getenv('MONGODB_URI')
    db_name = os.getenv('DB_NAME')
    client = MongoClient(mongodb_uri)
    db = client[db_name]
    collection = db['price']
    cursor = collection.find({"trader_name": "BSC-AlgoTrader"}).sort("price_point.timestamp", 1)
    data = pd.DataFrame(list(cursor))
    data['timestamp'] = data['price_point'].apply(lambda x: x['timestamp'])
    data['price'] = data['price_point'].apply(lambda x: x['price'])
    data['timestamp'] = pd.to_datetime(data['timestamp'], unit='s')
    data = data.pivot(index='timestamp', columns='token_name', values='price')
    data = data.fillna(method='ffill')
    return data

def create_features(data, past_data_points):
    all_lags = []
    for token in data.columns:
        lag_features = [data[token].shift(i) for i in range(1, past_data_points+1)]
        moving_average_feature = data[token].rolling(window=past_data_points).mean()
        rate_of_change_feature = data[token].pct_change()
        token_lags = pd.DataFrame({f'{token}_lag_{i}': feature for i, feature in enumerate(lag_features, start=1)}, index=data.index)
        token_lags[f'{token}_moving_average'] = moving_average_feature
        token_lags[f'{token}_rate_of_change'] = rate_of_change_feature
        all_lags.append(token_lags)
    features = pd.concat(all_lags, axis=1)
    features = features.iloc[past_data_points:]
    features = features.fillna(method='ffill')
    return features

def train_model(data, features, past_data_points):
    mongodb_uri = os.getenv('MONGODB_URI')
    db_name = os.getenv('DB_NAME')
    client = MongoClient(mongodb_uri)
    db = client[db_name]
    fs = gridfs.GridFS(db)
    model = RandomForestRegressor(n_estimators=100, max_depth=10)
    scaler = StandardScaler()
    X_train = scaler.fit_transform(features.values)
    y_train = data['WBNB'].iloc[past_data_points:].values
    total_data_points = len(X_train)
    model.fit(X_train, y_train)
    print(f"Training progress: 100.00%")
    binary_model = pickle.dumps(model)
    fs.put(binary_model, filename='model.pkl')
    binary_scaler = pickle.dumps(scaler)
    fs.put(binary_scaler, filename='scaler.pkl')

def predict(data, features, past_data_points, future_time_steps):
    mongodb_uri = os.getenv('MONGODB_URI')
    db_name = os.getenv('DB_NAME')
    client = MongoClient(mongodb_uri)
    db = client[db_name]
    fs = gridfs.GridFS(db)
    binary_model = fs.get_last_version(filename='model.pkl').read()
    model = pickle.loads(binary_model)
    binary_scaler = fs.get_last_version(filename='scaler.pkl').read()
    scaler = pickle.loads(binary_scaler)
    predictions = []
    total_steps = len(features) - future_time_steps
    for i in range(past_data_points, total_steps):
        X_new = scaler.transform(features.iloc[i:i+1].values)
        if i + future_time_steps < len(features):
            prediction = model.predict(X_new)
            predictions.append(prediction[0])
        progress_percentage = ((i - past_data_points) / (total_steps - past_data_points)) * 100
        # print(f"Prediction progress: {progress_percentage:.2f}%")
    return predictions[-1]

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Train and predict cryptocurrency prices')
    parser.add_argument('--mode', type=str, default='train', choices=['train', 'predict'], help='Mode to run the script in')
    parser.add_argument('--past_minutes', type=int, default=180, help='number of past minutes to consider')
    parser.add_argument('--future_minutes', type=int, default=180, help='number of short-term minutes into the future to predict')
    args = parser.parse_args()

    data = load_data_from_mongodb()
    past_data_points = args.past_minutes * 6
    features = create_features(data, past_data_points)

    if args.mode == 'train':
        train_model(data, features, past_data_points)
    elif args.mode == 'predict':
        future_minutes = args.future_minutes
        future_time_steps = future_minutes * 6
        predict(data, features, past_data_points, future_time_steps)
