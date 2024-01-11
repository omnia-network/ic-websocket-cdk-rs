use std::ops::Deref;

use crate::{errors::WsError, CanisterWsCloseArguments, CanisterWsCloseResult};

use super::utils::{
    actor::{
        wipe::call_wipe,
        ws_close::call_ws_close,
        ws_open::{
            call_ws_open_for_client_key_and_gateway_with_panic,
            call_ws_open_for_client_key_with_panic,
        },
    },
    clients::{CLIENT_1_KEY, CLIENT_2_KEY, GATEWAY_1, GATEWAY_2},
};

#[test]
fn test_1_fails_if_gateway_is_not_registered() {
    // first, reset the canister
    call_wipe(None);
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    let gateway_2_principal = GATEWAY_2.deref();

    // finally, we can start testing
    let res = call_ws_close(
        gateway_2_principal,
        CanisterWsCloseArguments::new(CLIENT_1_KEY.clone()),
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(
            WsError::GatewayNotRegistered {
                gateway_principal: gateway_2_principal
            }
            .to_string()
        ),
    );
}

#[test]
fn test_2_fails_if_client_is_not_registered() {
    // first, reset the canister
    call_wipe(None);
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    let client_2_key = CLIENT_2_KEY.deref();
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments::new(client_2_key.clone()),
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(
            WsError::ClientKeyNotConnected {
                client_key: client_2_key
            }
            .to_string()
        ),
    );
}

#[test]
fn test_3_fails_if_client_is_not_registered_to_gateway() {
    // first, reset the canister
    call_wipe(None);
    // second, open a connection for both clients
    call_ws_open_for_client_key_and_gateway_with_panic(CLIENT_1_KEY.deref(), *GATEWAY_1.deref());
    call_ws_open_for_client_key_and_gateway_with_panic(CLIENT_2_KEY.deref(), *GATEWAY_2.deref());

    let gateway_2_principal = GATEWAY_2.deref();

    // finally, we can start testing
    let res = call_ws_close(
        gateway_2_principal,
        CanisterWsCloseArguments::new(CLIENT_1_KEY.clone()),
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(
            WsError::ClientNotRegisteredToGateway {
                client_key: CLIENT_1_KEY.deref(),
                gateway_principal: gateway_2_principal
            }
            .to_string()
        ),
    );
}

#[test]
fn test_4_should_close_the_websocket_for_a_registered_client() {
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments::new(CLIENT_1_KEY.clone()),
    );
    assert_eq!(res, CanisterWsCloseResult::Ok(()));

    // we expect the ws_close to fail if we execute it again
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments::new(CLIENT_1_KEY.clone()),
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(
            WsError::ClientKeyNotConnected {
                client_key: &CLIENT_1_KEY,
            }
            .to_string()
        )
    )
}
