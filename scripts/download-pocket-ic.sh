#!/bin/bash

cd bin/

POCKET_IC_BIN=pocket-ic
POCKET_IC_VERSION=9.0.3

if [ -f "$POCKET_IC_BIN" ]; then
    echo -e "$POCKET_IC_BIN exists. Path: $(pwd)/$POCKET_IC_BIN\n"
else 
    echo "$POCKET_IC_BIN does not exist."

    echo "Downloading Pocket IC binary..."

    if [[ "$OSTYPE" == "linux"* ]]; then
        ARCH="linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        ARCH="darwin"
    else
        echo "Unsupported OS: $OSTYPE"
        exit 1
    fi

    CPU_ARCH=$(uname -m)
    if [[ "$CPU_ARCH" == "arm64" || "$CPU_ARCH" == "aarch64" ]]; then
        ARCH="arm64-${ARCH}"
    else
        ARCH="x86_64-${ARCH}"
    fi

    curl -sL -o $POCKET_IC_BIN.gz https://github.com/dfinity/pocketic/releases/download/$POCKET_IC_VERSION/pocket-ic-$ARCH.gz

    echo "Extracting Pocket IC binary..."
    gzip -d $POCKET_IC_BIN.gz
    chmod +x $POCKET_IC_BIN

    echo -e "Pocket IC binary downloaded and extracted successfully! Path: $(pwd)/$POCKET_IC_BIN\n"
fi
