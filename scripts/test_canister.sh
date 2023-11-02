#!/bin/bash

set -e

# unit tests
cargo test --package ic-websocket-cdk --lib -- tests::unit_tests

# integration tests
./scripts/download-pocket-ic.sh

./scripts/build-test-canister.sh

POCKET_IC_BIN=$(pwd)/bin/pocket-ic RUST_BACKTRACE=1 cargo test --package ic-websocket-cdk --lib -- tests::integration_tests --test-threads 1
