use std::ops::Deref;

use crate::{errors::WsError, CanisterSendResult};

use super::utils::{
    actor::{send::call_send, ws_open::call_ws_open_for_client_key_with_panic},
    clients::{CLIENT_1_KEY, CLIENT_2_KEY},
    messages::AppMessage,
    test_env::get_test_env,
};

#[test]
fn test_1_fails_if_sending_a_message_to_a_non_registered_client() {
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    // finally, we can start testing
    let client_2_principal = &CLIENT_2_KEY.client_principal;
    let res = call_send(
        client_2_principal,
        vec![AppMessage {
            text: String::from("test"),
        }],
    );
    assert_eq!(
        res,
        CanisterSendResult::Err(
            WsError::ClientPrincipalNotConnected {
                client_principal: client_2_principal
            }
            .to_string()
        ),
    );
}

#[test]
fn test_2_should_send_a_message_to_a_registered_client() {
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    let res = call_send(
        &CLIENT_1_KEY.client_principal,
        vec![AppMessage {
            text: String::from("test"),
        }],
    );
    assert_eq!(res, CanisterSendResult::Ok(()));
}
