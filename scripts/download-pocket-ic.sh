#!/bin/bash

cd bin/

POCKET_IC_BIN=pocket-ic
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
    curl -sL -o $POCKET_IC_BIN.gz https://github.com/dfinity/pocketic/releases/latest/download/pocket-ic-x86_64-$ARCH.gz

    echo "Extracting Pocket IC binary..."
    gzip -d $POCKET_IC_BIN.gz
    chmod +x $POCKET_IC_BIN

    echo -e "Pocket IC binary downloaded and extracted successfully! Path: $(pwd)/$POCKET_IC_BIN\n"
fi
