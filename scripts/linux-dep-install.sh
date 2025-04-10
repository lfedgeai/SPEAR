#!/bin/bash

# make sure the script is run as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root"
    exit
fi
# install dependencies
apt-get update

# install X11 libraries
apt-get install -y \
    libx11-dev \
    libxext-dev \
    libxrender-dev \
    libxtst-dev \
    pkg-config \
    libasound2-dev \
    libportaudio2 \
    portaudio19-dev

pip install build
