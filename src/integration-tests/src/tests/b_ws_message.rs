use std::ops::Deref;

use ic_websocket_cdk::{
    CanisterAckMessageContent, CanisterWsMessageArguments, CanisterWsMessageResult,
    ClientKeepAliveMessageContent, ClientKey, WebsocketServiceMessageContent,
};

use crate::{
    actor::{ws_message::call_ws_message, ws_open::call_ws_open_for_client_key_with_panic},
    clients::{generate_random_client_nonce, CLIENT_1_KEY, CLIENT_2, CLIENT_2_KEY},
    constants::{
        DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS, DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
        DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
    },
    messages::{create_websocket_message, encode_websocket_service_message_content},
    TEST_ENV,
};

#[test]
fn test_1_fails_if_client_is_not_registered() {
    // first, reset the canister
    TEST_ENV.reset_canister(
        DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
        DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
        DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
    );
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
        CanisterWsMessageResult::Err(String::from(format!(
            "client with principal {} doesn't have an open connection",
            client_2_key.client_principal,
        ))),
    );
}

#[test]
fn test_2_fails_if_client_sends_a_message_with_a_different_client_key() {
    let client_1_key = CLIENT_1_KEY.deref();

    // first, send a message with a different principal
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(
                &ClientKey {
                    client_principal: *CLIENT_2.deref(),
                    ..client_1_key.clone()
                },
                0,
                None,
                false,
            ),
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(String::from(format!(
            "client with principal {} has a different key than the one used in the message",
            client_1_key.client_principal,
        )))
    );

    // then, send a message with a different nonce
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(
                &ClientKey {
                    client_nonce: generate_random_client_nonce(),
                    ..client_1_key.clone()
                },
                0,
                None,
                false,
            ),
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(String::from(format!(
            "client with principal {} has a different key than the one used in the message",
            client_1_key.client_principal,
        )))
    );
}

#[test]
fn test_3_should_send_a_message_from_a_registered_client() {
    let client_1_key = CLIENT_1_KEY.deref();
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
    let wrong_sequence_number = 1; // the message with sequence number 1 has already been sent in the previous test
    let expected_sequence_number = 2; // the next valid sequence number
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(client_1_key, wrong_sequence_number, None, false),
        },
    );
    assert_eq!(res, CanisterWsMessageResult::Err(String::from(
        format!("incoming client's message does not have the expected sequence number. Expected: {expected_sequence_number}, actual: {wrong_sequence_number}. Client removed."))
    ));

    // check if the client has been removed
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(client_1_key, 1, None, false), // the sequence number doesn't matter here because the method fails before checking it
        },
    );
    assert_eq!(
        res,
        CanisterWsMessageResult::Err(String::from(format!(
            "client with principal {} doesn't have an open connection",
            client_1_key.client_principal,
        ),))
    )
}

#[test]
fn test_5_fails_if_client_sends_a_wrong_service_message() {
    let client_1_key = CLIENT_1_KEY.deref();
    // first, open the connection again for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    // fail with wrong content encoding
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(client_1_key, 1, Some(vec![1, 2, 3]), true),
        },
    );
    match res {
        CanisterWsMessageResult::Err(err) => {
            assert!(err.starts_with("Error decoding service message content:"))
        },
        _ => panic!("unexpected result"),
    };

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
        CanisterWsMessageResult::Err(String::from("Invalid received service message"))
    );
}

#[test]
fn test_6_should_send_a_service_message_from_a_registered_client() {
    let client_1_key = CLIENT_1_KEY.deref();
    let client_service_message =
        WebsocketServiceMessageContent::KeepAliveMessage(ClientKeepAliveMessageContent {
            last_incoming_sequence_num: 0,
        });
    let res = call_ws_message(
        &client_1_key.client_principal,
        CanisterWsMessageArguments {
            msg: create_websocket_message(
                client_1_key,
                3,
                Some(encode_websocket_service_message_content(
                    &client_service_message,
                )),
                true,
            ),
        },
    );
    assert_eq!(res, CanisterWsMessageResult::Ok(()));
}
