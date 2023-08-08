#!/bin/bash

set -e

# unit tests
cargo test -p ic-websocket-cdk

cd tests

npm install

# integration tests
dfx start --clean --background

IC_WS_CDK_INTEGRATION_TEST=1 npm run deploy:tests

npm run generate

npm run test:integration

dfx stop
