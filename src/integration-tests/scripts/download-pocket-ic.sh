#!/bin/bash

cd bin/

echo "Downloading Pocket IC binary..."
curl -sLO https://download.dfinity.systems/ic/307d5847c1d2fe1f5e19181c7d0fcec23f4658b3/openssl-static-binaries/x86_64-linux/pocket-ic.gz

echo "Extracting Pocket IC binary..."
gzip -d pocket-ic.gz
chmod +x pocket-ic

echo "Pocket IC binary downloaded and extracted successfully! Path: $(pwd)/pocket-ic"
