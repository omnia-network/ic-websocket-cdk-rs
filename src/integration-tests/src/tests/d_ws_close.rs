use std::ops::Deref;

use ic_websocket_cdk::{CanisterWsCloseArguments, CanisterWsCloseResult};

use crate::{
    actor::{ws_close::call_ws_close, ws_open::call_ws_open_for_client_key_with_panic},
    clients::{CLIENT_1_KEY, CLIENT_2_KEY, GATEWAY_1, GATEWAY_2},
    TEST_ENV,
};

#[test]
fn test_1_fails_if_gateway_is_not_registered() {
    // first, reset the canister
    TEST_ENV.reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    // finally, we can start testing
    let res = call_ws_close(
        GATEWAY_2.deref(),
        CanisterWsCloseArguments {
            client_key: CLIENT_1_KEY.clone(),
        },
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(String::from(
            "caller is not the gateway that has been registered during CDK initialization",
        )),
    );
}

#[test]
fn test_2_fails_if_client_is_not_registered() {
    let client_2_key = CLIENT_2_KEY.deref();
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments {
            client_key: client_2_key.clone(),
        },
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(String::from(format!(
            "client with key {client_2_key} doesn't have an open connection"
        ))),
    );
}

#[test]
fn test_3_should_close_the_websocket_for_a_registered_client() {
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments {
            client_key: CLIENT_1_KEY.clone(),
        },
    );
    assert_eq!(res, CanisterWsCloseResult::Ok(()));
}
