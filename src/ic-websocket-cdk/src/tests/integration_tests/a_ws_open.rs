use std::ops::Deref;

use crate::{
    CanisterOutputMessage, CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult,
    CanisterWsOpenArguments, CanisterWsOpenResult, ClientKey, WebsocketServiceMessageContent,
};
use candid::Principal;

use super::utils::{
    actor::{ws_get_messages::call_ws_get_messages, ws_open::call_ws_open},
    clients::{generate_random_client_nonce, CLIENT_1, CLIENT_1_KEY, GATEWAY_1},
    messages::get_service_message_content_from_canister_message,
};

#[test]
fn test_1_fail_for_an_anonymous_client() {
    let args = CanisterWsOpenArguments {
        client_nonce: generate_random_client_nonce(),
    };
    let res = call_ws_open(&Principal::anonymous(), args);
    assert_eq!(
        res,
        CanisterWsOpenResult::Err(String::from("anonymous principal cannot open a connection")),
    );
}

#[test]
fn test_2_fails_for_the_registered_gateway() {
    let args = CanisterWsOpenArguments {
        client_nonce: generate_random_client_nonce(),
    };
    let res = call_ws_open(GATEWAY_1.deref(), args);
    assert_eq!(
        res,
        CanisterWsOpenResult::Err(String::from(
            "caller is the registered gateway which can't open a connection for itself",
        )),
    );
}

#[test]
fn test_3_should_open_a_connection() {
    let client_1_key = CLIENT_1_KEY.deref();
    let args = CanisterWsOpenArguments {
        client_nonce: client_1_key.client_nonce,
    };
    let res = call_ws_open(CLIENT_1.deref(), args);
    assert_eq!(res, CanisterWsOpenResult::Ok(()));

    let msgs = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );

    match msgs {
        CanisterWsGetMessagesResult::Ok(messages) => {
            let first_message = &messages.messages[0];
            assert_eq!(first_message.client_key, *client_1_key);
            let open_message = get_service_message_content_from_canister_message(first_message);
            match open_message {
                WebsocketServiceMessageContent::OpenMessage(open_message) => {
                    assert_eq!(open_message.client_key, *client_1_key);
                },
                _ => panic!("Expected OpenMessage"),
            }
        },
        _ => panic!("Expected Ok result"),
    }
}

#[test]
fn test_4_fails_for_a_client_with_the_same_nonce() {
    let client_1_key = CLIENT_1_KEY.deref();
    let args = CanisterWsOpenArguments {
        client_nonce: client_1_key.client_nonce,
    };
    let res = call_ws_open(CLIENT_1.deref(), args);
    assert_eq!(
        res,
        CanisterWsOpenResult::Err(String::from(format!(
            "client with key {client_1_key} already has an open connection"
        ))),
    );
}

#[test]
fn test_5_should_open_a_connection_for_the_same_client_with_a_different_nonce() {
    let client_key = ClientKey {
        client_principal: CLIENT_1_KEY.deref().client_principal,
        client_nonce: generate_random_client_nonce(),
    };
    let args = CanisterWsOpenArguments {
        client_nonce: client_key.client_nonce,
    };
    let res = call_ws_open(&client_key.client_principal, args);
    assert_eq!(res, CanisterWsOpenResult::Ok(()));

    let msgs = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );

    match msgs {
        CanisterWsGetMessagesResult::Ok(messages) => {
            let service_message_for_client = messages
                .messages
                .iter()
                .filter(|msg| msg.client_key == client_key)
                .collect::<Vec<&CanisterOutputMessage>>()[0];

            let open_message =
                get_service_message_content_from_canister_message(service_message_for_client);
            match open_message {
                WebsocketServiceMessageContent::OpenMessage(open_message) => {
                    assert_eq!(open_message.client_key, client_key);
                },
                _ => panic!("Expected OpenMessage"),
            }
        },
        _ => panic!("Expected Ok result"),
    }
}
