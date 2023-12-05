use std::ops::Deref;

use candid::encode_one;

use crate::{
    errors::WsError,
    tests::{
        common::generate_random_principal,
        integration_tests::utils::{
            actor::{ws_close::call_ws_close, ws_get_messages::call_ws_get_messages_with_panic},
            clients::GATEWAY_1,
            messages::check_canister_message_has_close_reason,
        },
    },
    types::CloseMessageReason,
    CanisterAckMessageContent, CanisterWsCloseArguments, CanisterWsCloseResult,
    CanisterWsGetMessagesArguments, CanisterWsMessageArguments, CanisterWsMessageResult,
    ClientKeepAliveMessageContent, ClientKey, WebsocketServiceMessageContent,
};

use super::utils::{
    actor::{ws_message::call_ws_message, ws_open::call_ws_open_for_client_key_with_panic},
    clients::{generate_random_client_nonce, CLIENT_1_KEY, CLIENT_2_KEY},
    messages::{create_websocket_message, encode_websocket_service_message_content},
    test_env::get_test_env,
};

#[test]
fn test_1_fails_if_client_is_not_registered() {
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(CLIENT_1_KEY.deref());

    let client_2_key = CLIENT_2_KEY.deref();
    let res = call_ws_message(
        &client_2_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(client_2_key, 0, None, false),
        },
    );

    assert_eq!(
        res,
        CanisterWsMessageResult::Err(
            WsError::ClientPrincipalNotConnected {
                client_principal: &client_2_key.client_principal
            }
            .to_string()
        ),
    );
}

#[test]
fn test_2_fails_if_client_sends_a_message_with_a_different_client_key() {
    let client_1_key = CLIENT_1_KEY.deref();
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    let wrong_client_key = ClientKey {
        client_principal: generate_random_principal(),
        ..client_1_key.clone()
    };

    // first, send a message with a different principal
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(&wrong_client_key, 0, None, false),
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(
            WsError::ClientKeyMessageMismatch {
                client_key: &wrong_client_key
            }
            .to_string()
        )
    );

    let wrong_client_key = ClientKey {
        client_nonce: generate_random_client_nonce(),
        ..client_1_key.clone()
    };

    // then, send a message with a different nonce
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(&wrong_client_key, 0, None, false),
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(
            WsError::ClientKeyMessageMismatch {
                client_key: &wrong_client_key
            }
            .to_string()
        )
    );
}

#[test]
fn test_3_should_send_a_message_from_a_registered_client() {
    let client_1_key = CLIENT_1_KEY.deref();
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(client_1_key, 1, None, false),
        },
    );
    assert_eq!(res, CanisterWsMessageResult::Ok(()));
}

#[test]
fn test_4_fails_if_client_sends_a_message_with_a_wrong_sequence_number() {
    let client_1_key = CLIENT_1_KEY.deref();
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    let wrong_sequence_number = 2; // the message with sequence number 1 has already been sent in the previous test
    let expected_sequence_number = 1; // the next valid sequence number
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(client_1_key, wrong_sequence_number, None, false),
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(
            WsError::IncomingSequenceNumberWrong {
                expected_sequence_num: expected_sequence_number,
                actual_sequence_num: wrong_sequence_number
            }
            .to_string()
        )
    );

    // check if the gateway put the close message in the queue
    let msgs = call_ws_get_messages_with_panic(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 1 }, // skip the first open message
    );
    check_canister_message_has_close_reason(
        &msgs.messages[0],
        CloseMessageReason::WrongSequenceNumber,
    );

    // the gateway should still be between the registered gateways
    // so calling the ws_close endpoint should return the ClientKeyNotConnected error
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments {
            client_key: client_1_key.clone(),
        },
    );
    assert_eq!(
        res,
        CanisterWsCloseResult::Err(
            WsError::ClientKeyNotConnected {
                client_key: &client_1_key
            }
            .to_string()
        )
    );
}

#[test]
fn test_5_fails_if_client_sends_a_wrong_service_message() {
    let client_1_key = CLIENT_1_KEY.deref();
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    // fail with wrong content encoding
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(
                client_1_key,
                1,
                Some(encode_one(vec![1, 2, 3]).unwrap()),
                true,
            ),
        },
    );
    assert!(res
        .err()
        .unwrap()
        .starts_with("Error decoding service message content:"));

    // fail with wrong service message variant
    let wrong_service_message =
        WebsocketServiceMessageContent::AckMessage(CanisterAckMessageContent {
            last_incoming_sequence_num: 0,
        });
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(
                client_1_key,
                2,
                Some(encode_websocket_service_message_content(
                    &wrong_service_message,
                )),
                true,
            ),
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(WsError::InvalidServiceMessage.to_string())
    );
}

#[test]
fn test_6_should_send_a_service_message_from_a_registered_client() {
    let client_1_key = CLIENT_1_KEY.deref();
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    let client_service_message =
        WebsocketServiceMessageContent::KeepAliveMessage(ClientKeepAliveMessageContent {
            last_incoming_sequence_num: 0,
        });
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(
                client_1_key,
                1,
                Some(encode_websocket_service_message_content(
                    &client_service_message,
                )),
                true,
            ),
        },
    );
    assert_eq!(res, CanisterWsMessageResult::Ok(()));
}
