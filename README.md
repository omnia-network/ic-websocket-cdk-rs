# ic-websocket-cdk-rs

This repository contains the Rust implementation of IC WebSocket CDK. For more information about IC Websockets, see [ic-websocket](https://github.com/omnia-network/ic-websocket).

## Usage

TODO: Add usage instructions

## Development

### Testing

There are two types of tests available:
- **Unit tests**: tests for CDK functions, written in Rust.
- **Integration tests**: for these tests a local IC replica is set up and the CDK is deployed to a [test canister](./tests/src/lib.rs). Tests are written in Node.js and are available in the [tests](./tests/integration/) folder.

There's a script that runs all the tests together, taking care of setting up the replica and deploying the canister. To run the script, execute the following command:

```bash
./scripts/test_canister.sh
```
