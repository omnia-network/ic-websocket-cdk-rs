#!/bin/bash

set -e

# unit tests
cargo test --package ic-websocket-cdk --lib -- tests::unit_tests
# doc tests
cargo test --package ic-websocket-cdk --doc

# integration tests
./scripts/download-pocket-ic.sh

./scripts/build-test-canister.sh

POCKET_IC_MUTE_SERVER=1 POCKET_IC_BIN="$(pwd)/bin/pocket-ic" cargo test --package ic-websocket-cdk --lib -- tests::integration_tests --test-threads 1
