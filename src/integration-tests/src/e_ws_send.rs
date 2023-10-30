use std::ops::Deref;

use ic_websocket_cdk::CanisterWsSendResult;

use crate::utils::{
    actor::{
        ws_open::call_ws_open_for_client_key_with_panic,
        ws_send::{call_ws_send, AppMessage},
    },
    clients::{CLIENT_1, CLIENT_1_KEY, CLIENT_2},
    test_env::TEST_ENV,
};

#[test]
fn test_1_fails_if_sending_a_message_to_a_non_registered_client() {
    // first, reset the canister
    TEST_ENV.reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    // finally, we can start testing
    let client_2_principal = CLIENT_2.deref();
    let res = call_ws_send(
        client_2_principal,
        vec![AppMessage {
            text: String::from("test"),
        }],
    );
    assert_eq!(
        res,
        CanisterWsSendResult::Err(String::from(format!(
            "client with principal {client_2_principal} doesn't have an open connection"
        ))),
    );
}

#[test]
fn test_2_should_send_a_message_to_a_registered_client() {
    let res = call_ws_send(
        CLIENT_1.deref(),
        vec![AppMessage {
            text: String::from("test"),
        }],
    );
    assert_eq!(res, CanisterWsSendResult::Ok(()));
}
