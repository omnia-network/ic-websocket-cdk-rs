#!/bin/bash

cd bin/

POCKET_IC_BIN=pocket-ic
if [ -f "$POCKET_IC_BIN" ]; then
    echo -e "$POCKET_IC_BIN exists. Path: $(pwd)/$POCKET_IC_BIN\n"
else 
    echo "$POCKET_IC_BIN does not exist."
    echo "Downloading Pocket IC binary..."
    curl -sLO https://download.dfinity.systems/ic/307d5847c1d2fe1f5e19181c7d0fcec23f4658b3/openssl-static-binaries/x86_64-linux/pocket-ic.gz

    echo "Extracting Pocket IC binary..."
    gzip -d $POCKET_IC_BIN.gz
    chmod +x $POCKET_IC_BIN

    echo -e "Pocket IC binary downloaded and extracted successfully! Path: $(pwd)/$POCKET_IC_BIN\n"
fi
