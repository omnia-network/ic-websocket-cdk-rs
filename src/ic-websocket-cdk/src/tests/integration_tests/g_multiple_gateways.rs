use std::ops::Deref;

use crate::{
    tests::integration_tests::utils::messages::get_service_message_content_from_canister_message,
    CanisterOutputCertifiedMessages, CanisterWsCloseArguments, CanisterWsGetMessagesArguments,
    CanisterWsGetMessagesResult, WebsocketServiceMessageContent,
};

use super::utils::{
    actor::{
        ws_close::call_ws_close_with_panic,
        ws_get_messages::call_ws_get_messages,
        ws_open::call_ws_open_for_client_key_and_gateway_with_panic,
        ws_send::{call_ws_send_with_panic, AppMessage},
    },
    clients::{CLIENT_1_KEY, GATEWAY_1, GATEWAY_2},
    test_env::get_test_env,
};

#[test]
fn test_1_client_can_switch_to_another_gateway() {
    get_test_env().reset_canister_with_gateways(vec![GATEWAY_1.to_string(), GATEWAY_2.to_string()]);
    // open a connection for client 1
    let client_1_key = CLIENT_1_KEY.deref();
    call_ws_open_for_client_key_and_gateway_with_panic(client_1_key, *GATEWAY_1);
    // simulate canister sending messages to client
    call_ws_send_with_panic(
        &client_1_key.client_principal,
        (0..10)
            .map(|i| AppMessage {
                text: format!("test{}", i),
            })
            .collect(),
    );

    // test
    // gateway 1 can poll the messages
    let res_gateway_1 = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );
    match res_gateway_1 {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
            assert_eq!(messages.len() as u64, 10 + 1); // +1 for the open service message
        },
        _ => panic!("unexpected result"),
    };
    // gateway 2 has no messages
    let res_gateway_2 = call_ws_get_messages(
        GATEWAY_2.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );
    match res_gateway_2 {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
            assert_eq!(messages.len() as u64, 0);
        },
        _ => panic!("unexpected result"),
    };

    // client disconnects, so gateway 1 closes the connection
    call_ws_close_with_panic(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments {
            client_key: client_1_key.clone(),
        },
    );
    // client reopens connection with gateway 2
    call_ws_open_for_client_key_and_gateway_with_panic(client_1_key, *GATEWAY_2);
    // gateway 2 now has the open message
    let res_gateway_2 = call_ws_get_messages(
        GATEWAY_2.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );
    match res_gateway_2 {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
            let first_message = &messages[0];
            assert_eq!(first_message.client_key, *client_1_key);
            let open_message = get_service_message_content_from_canister_message(first_message);
            match open_message {
                WebsocketServiceMessageContent::OpenMessage(open_message) => {
                    assert_eq!(open_message.client_key, *client_1_key);
                },
                _ => panic!("Expected OpenMessage"),
            }
        },
        _ => panic!("unexpected result"),
    };

    // simulate canister sending other messages to client
    call_ws_send_with_panic(
        &client_1_key.client_principal,
        (0..10)
            .map(|i| AppMessage {
                text: format!("test{}", i + 10),
            })
            .collect(),
    );
    let res_gateway_2 = call_ws_get_messages(
        GATEWAY_2.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );
    match res_gateway_2 {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
            assert_eq!(messages.len() as u64, 10 + 1); // +1 for the open service message
        },
        _ => panic!("unexpected result"),
    };
}
