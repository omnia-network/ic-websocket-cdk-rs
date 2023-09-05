use ic_cdk_macros::*;

use canister::{on_close, on_message, on_open};
use ic_websocket_cdk::{
    CanisterWsCloseArguments, CanisterWsCloseResult, CanisterWsGetMessagesArguments,
    CanisterWsGetMessagesResult, CanisterWsMessageArguments, CanisterWsMessageResult,
    CanisterWsOpenArguments, CanisterWsOpenResult, CanisterWsSendResult, CanisterWsStatusArguments,
    CanisterWsStatusResult, ClientPrincipal, WsHandlers, WsInitParams,
};

mod canister;

#[init]
fn init(gateway_principal: String) {
    let handlers = WsHandlers {
        on_open: Some(on_open),
        on_message: Some(on_message),
        on_close: Some(on_close),
    };

    let params = WsInitParams {
        handlers,
        gateway_principal,
        check_registered_gateway_interval_ms: 15_000,
        send_ack_interval_ms: 10_000,
    };

    ic_websocket_cdk::init(params)
}

#[post_upgrade]
fn post_upgrade(gateway_principal: String) {
    init(gateway_principal);
}

// method called by the WS Gateway after receiving FirstMessage from the client
#[update]
fn ws_open(args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
    ic_websocket_cdk::ws_open(args)
}

// method called by the Ws Gateway when closing the IcWebSocket connection
#[update]
fn ws_close(args: CanisterWsCloseArguments) -> CanisterWsCloseResult {
    ic_websocket_cdk::ws_close(args)
}

// method called by the WS Gateway to send a message of type GatewayMessage to the canister
#[update]
fn ws_message(args: CanisterWsMessageArguments) -> CanisterWsMessageResult {
    ic_websocket_cdk::ws_message(args)
}

// method called by the WS Gateway to update its status in the canister
#[update]
fn ws_status(args: CanisterWsStatusArguments) -> CanisterWsStatusResult {
    ic_websocket_cdk::ws_status(args)
}

// method called by the WS Gateway to get messages for all the clients it serves
#[query]
fn ws_get_messages(args: CanisterWsGetMessagesArguments) -> CanisterWsGetMessagesResult {
    ic_websocket_cdk::ws_get_messages(args)
}

//// Debug/tests methods
// wipe all websocket data in the canister
#[update]
fn ws_wipe() {
    ic_websocket_cdk::wipe();
}

// send a message to the client, usually called by the canister itself
#[update]
fn ws_send(client_principal: ClientPrincipal, msg_bytes: Vec<u8>) -> CanisterWsSendResult {
    ic_websocket_cdk::ws_send(client_principal, msg_bytes)
}
