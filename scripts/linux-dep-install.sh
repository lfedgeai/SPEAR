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
    portaudio19-dev \
    cmake dkms \
    net-tools isort

pip install build websocket-client

set -e
# install FlatBuffers
git clone https://github.com/google/flatbuffers.git
cd flatbuffers
cmake -G "Unix Makefiles"
make -j4 #Compile with 4 threads
sudo make install #install
sudo ldconfig #Configuring a dynamic link library
flatc --version #Check if FlatBuffers is installed successfully
