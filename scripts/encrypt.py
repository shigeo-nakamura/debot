#!/usr/bin/env python3

import base64
from Crypto.Cipher import AES
from Crypto.Util.Padding import pad
import argparse

# Define the program description
parser = argparse.ArgumentParser(description='Encrypt data using an AES key.')

# Define arguments
parser.add_argument('aes_key', type=str,
                    help='The plaintext data key you got from AWS KMS')
parser.add_argument('data_to_encrypt', type=str,
                    help='The data you want to encrypt')
parser.add_argument('--hex', action='store_true',
                    help='Indicate that the data to encrypt is in hexadecimal format')

# Parse arguments
args = parser.parse_args()

# This is the plaintext data key you got from AWS KMS
aes_key = base64.b64decode(args.aes_key)

# Convert the data to bytes. If data starts with '0x' or --hex is provided, treat it as a hex string.
if args.data_to_encrypt.startswith('0x') or args.hex:
    data = bytes.fromhex(args.data_to_encrypt[2:] if args.data_to_encrypt.startswith(
        '0x') else args.data_to_encrypt)
else:
    data = args.data_to_encrypt.encode()

cipher = AES.new(aes_key, AES.MODE_CBC)
ciphertext = cipher.encrypt(pad(data, AES.block_size))

# Combine the iv (initialization vector) and the ciphertext, which you need both to decrypt the data
encrypted_data = base64.b64encode(cipher.iv + ciphertext).decode('utf-8')

print(encrypted_data)
