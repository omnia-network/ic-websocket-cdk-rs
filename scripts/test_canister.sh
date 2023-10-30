#!/bin/bash

set -e

# unit tests
cargo test -p ic-websocket-cdk

# integration tests
cd src/integration-tests

./scripts/download-pocket-ic.sh

./scripts/build-test-canister.sh

POCKET_IC_BIN=./bin/pocket-ic RUST_BACKTRACE=1 cargo test -p integration-tests -- --test-threads 1
