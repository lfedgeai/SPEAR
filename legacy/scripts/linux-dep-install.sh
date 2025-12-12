#!/bin/bash

# make sure the script is run as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root"
    exit
fi

set -e

ARCH=$(dpkg --print-architecture 2>/dev/null || uname -m)
echo "Detected architecture: ${ARCH}"

apt-get update

# install X11 and audio development libraries
apt-get install -y \
    build-essential \
    cmake dkms \
    pkg-config \
    libx11-dev \
    libxext-dev \
    libxrender-dev \
    libxtst-dev \
    libasound2-dev \
    libportaudio2 \
    portaudio19-dev \
    net-tools

PIP_ARGS="--break-system-packages --no-cache-dir"
python3 -m pip install ${PIP_ARGS} --ignore-installed pip
python3 -m pip install ${PIP_ARGS} build websocket-client isort

if ! command -v flatc >/dev/null 2>&1; then
    case "$ARCH" in
        amd64|x86_64)
            echo "Installing flatbuffers-compiler from apt for amd64..."
            apt-get install -y flatbuffers-compiler
            ;;
        arm64|aarch64)
            echo "Attempting to install flatbuffers-compiler from apt for ARM64..."
            if ! apt-get install -y flatbuffers-compiler; then
                echo "Apt installation failed; building FlatBuffers from source for ARM64..."
                TMP_DIR=$(mktemp -d)
                git clone https://github.com/google/flatbuffers.git "$TMP_DIR/flatbuffers"
                pushd "$TMP_DIR/flatbuffers"
                cmake -G "Unix Makefiles"
                make -j"$(nproc)"
                sudo make install
                sudo ldconfig
                popd
                rm -rf "$TMP_DIR"
            fi
            ;;
        *)
            TMP_DIR=$(mktemp -d)
            git clone https://github.com/google/flatbuffers.git "$TMP_DIR/flatbuffers"
            pushd "$TMP_DIR/flatbuffers"
            cmake -G "Unix Makefiles"
            make -j"$(nproc)"
            sudo make install
            sudo ldconfig
            popd
            rm -rf "$TMP_DIR"
            ;;
    esac
fi

# ensure installed flatc supports go module flag (requires >= 23.5)
if ! flatc --help 2>/dev/null | grep -q -- "--go-module-name"; then
    echo "Installed flatc lacks --go-module-name; building latest FlatBuffers from source..."
    TMP_DIR=$(mktemp -d)
    git clone https://github.com/google/flatbuffers.git "$TMP_DIR/flatbuffers"
    pushd "$TMP_DIR/flatbuffers"
    cmake -G "Unix Makefiles"
    make -j"$(nproc)"
    sudo make install
    sudo ldconfig
    popd
    rm -rf "$TMP_DIR"
fi

flatc --version
