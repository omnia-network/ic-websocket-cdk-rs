use std::ops::Deref;

use crate::{errors::WsError, CanisterCloseResult};

use super::utils::{
    actor::{close::call_close, wipe::call_wipe, ws_open::call_ws_open_for_client_key_with_panic},
    clients::{CLIENT_1_KEY, CLIENT_2_KEY},
};

#[test]
fn test_1_fails_closing_for_non_registered_client() {
    // first, reset the canister
    call_wipe(None);
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    // finally, we can start testing
    let client_2_principal = &CLIENT_2_KEY.client_principal;
    let res = call_close(client_2_principal);
    assert_eq!(
        res,
        CanisterCloseResult::Err(
            WsError::ClientPrincipalNotConnected {
                client_principal: client_2_principal
            }
            .to_string()
        ),
    );
}

#[test]
fn test_2_should_close_connection_for_registered_client() {
    // first, reset the canister
    call_wipe(None);
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    // finally, we can start testing
    let client_1_principal = &CLIENT_1_KEY.client_principal;
    let res = call_close(client_1_principal);
    assert_eq!(res, CanisterCloseResult::Ok(()));
    // call the same function again, and expect it to fail
    let res = call_close(client_1_principal);
    assert_eq!(
        res,
        CanisterCloseResult::Err(
            WsError::ClientPrincipalNotConnected {
                client_principal: client_1_principal
            }
            .to_string()
        ),
    );
}
