#!/usr/bin/python3

import base64
from Crypto.Cipher import AES
from Crypto.Util.Padding import pad
import argparse

# Define the program description
parser = argparse.ArgumentParser(description='Encrypt a private key using an AES key.')

# Define arguments
parser.add_argument('aes_key', type=str, help='The plaintext data key you got from AWS KMS')
parser.add_argument('private_key_hex', type=str, help='Your private key as a hexadecimal string')

# Parse arguments
args = parser.parse_args()

# This is the plaintext data key you got from AWS KMS
aes_key = base64.b64decode(args.aes_key)

# This is the data you want to encrypt, e.g. your private key
data = bytes.fromhex(args.private_key_hex)

cipher = AES.new(aes_key, AES.MODE_CBC)
ciphertext = cipher.encrypt(pad(data, AES.block_size))

# Combine the iv (initialization vector) and the ciphertext, which you need both to decrypt the data
encrypted_data = base64.b64encode(cipher.iv + ciphertext).decode('utf-8')

print(encrypted_data)

