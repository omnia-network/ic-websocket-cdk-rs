# ic-websocket-cdk-rs

This repository contains the Rust implementation of IC WebSocket CDK. For more information about IC WebSockets, see [IC WebSocket Gateway](https://github.com/omnia-network/ic-websocket-gateway).

## Installation

You can install the library by running:

```bash
cargo add ic-websocket-cdk
```

## Usage

Refer to the [ic_websocket_example](https://github.com/omnia-network/ic_websocket_example) repository for an example of how to use the library.

### Candid interface
In order for the frontend clients and the Gateway to work properly, the canister must expose some specific methods in its Candid interface, between the custom methods that you've implemented for your logic. A valid Candid interface for the canister is the following:

```
import "./ws_types.did";

// define here your message type
type MyMessageType = {
  some_field : text;
};

service : {
  "ws_open" : (CanisterWsOpenArguments) -> (CanisterWsOpenResult);
  "ws_close" : (CanisterWsCloseArguments) -> (CanisterWsCloseResult);
  "ws_message" : (CanisterWsMessageArguments, opt MyMessageType) -> (CanisterWsMessageResult);
  "ws_get_messages" : (CanisterWsGetMessagesArguments) -> (CanisterWsGetMessagesResult) query;
};
```
This snipped is copied from the [service.example.did](./src/ic-websocket-cdk/service.example.did) file and the types imported are defined in the [ws_types.did](./src/ic-websocket-cdk/ws_types.did) file.

To define your message type, you can use the [Candid reference docs](https://internetcomputer.org/docs/current/references/candid-ref). We suggest you to define your message type using a [variant](https://internetcomputer.org/docs/current/references/candid-ref#type-variant--n--t--), so that you can support different messages over the same websocket instance and make it safe for future updates of your types.

To automatically generate the Candid interfaces for your canister, have a look at [this article](https://daviddalbusco.com/blog/automatic-candid-generation-in-rust-exploring-the-ic-cdk-v0-10-0-update/).

## Development

The **ic-websocket-cdk** library implementation can be found in the [src/ic-websocket-cdk](./src/ic-websocket-cdk/) folder.

### Testing

There are two types of tests available:
- **Unit tests**: tests for CDK functions, written in Rust and available in the [unit_tests](./src/ic-websocket-cdk/src/tests/unit_tests/) folder.
- **Integration tests**: for these tests the CDK is deployed to a [test canister](./src/test_canister/). These tests are written in Rust and use [PocketIC](https://github.com/dfinity/pocketic) under the hood. They are available in the [integration_tests](./src/ic-websocket-cdk/src/tests/integration_tests/) folder.

There's a script that runs all the tests together, taking care of setting up the environment (Linux only!) and deploying the canister. To run the script, execute the following command:

```bash
./scripts/test.sh
```

> If you're on **macOS**, you have to manually download the PocketIC binary ([guide](https://github.com/dfinity/pocketic#download)) and place it in the [bin](./bin/) folder.

## License

MIT License. See [LICENSE](./LICENSE).

## Contributing

Feel free to open issues and pull requests.
