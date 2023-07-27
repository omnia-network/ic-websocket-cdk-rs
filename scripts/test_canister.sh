#!/bin/bash

set -e

# unit tests
cargo test

cd tests

npm install

# integration tests
dfx start --clean --background

npm run deploy:tests

npx jest integration/canister

dfx stop
