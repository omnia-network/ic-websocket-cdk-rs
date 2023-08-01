# ic-websocket-cdk-rs

This repository contains the Rust implementation of IC WebSocket CDK. For more information about IC Websockets, see [IC WebSocket Gateway](https://github.com/omnia-network/ic-websocket-gateway).

## Installation

You can install the library by adding the following line to your `Cargo.toml` file:

```toml
ic-websocket-cdk = { git = "https://github.com/omnia-network/ic-websocket-cdk-rs", rev = "<last-commit-on-this-repo>" }
```

For example, a valid installation line would be:

```toml
ic-websocket-cdk = { git = "https://github.com/omnia-network/ic-websocket-cdk-rs", rev = "989f74b6f871c113628cd20697e42b85d503e37d" }
```

It will also be available on crates.io soon.

## Usage

Refer to the [ic_websocket_example](https://github.com/omnia-network/ic_websocket_example) repository for an example of how to use the library.

## Development

The **ic-websocket-cdk** library implementation can be found in the [src/ic-websocket-cdk](./src/ic-websocket-cdk/) folder.

### Testing

There are two types of tests available:
- **Unit tests**: tests for CDK functions, written in Rust.
- **Integration tests**: for these tests a local IC replica is set up and the CDK is deployed to a [test canister](./tests/src/lib.rs). Tests are written in Node.js and are available in the [tests](./tests/integration/) folder.

There's a script that runs all the tests together, taking care of setting up the replica and deploying the canister. To run the script, execute the following command:

```bash
./scripts/test_canister.sh
```

## License

TODO: Add a license

## Contributing

Feel free to open issues and pull requests.
