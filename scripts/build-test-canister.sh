#!/bin/bash

cargo build --locked --target wasm32-unknown-unknown --release --package test_canister
