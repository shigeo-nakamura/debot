#!/bin/bash

base_dir="/home/ec2-user"

if [ -z "$1" ]; then
    echo "Error: No environment argument provided."
    exit 1
fi

source ${base_dir}/debot/scripts/$1.env

${base_dir}/debot/target/release/debot

