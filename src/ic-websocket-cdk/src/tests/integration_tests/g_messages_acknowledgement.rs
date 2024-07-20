use std::ops::Deref;

use crate::{
    errors::WsError,
    tests::integration_tests::utils::{
        actor::ws_close::call_ws_close, messages::check_canister_message_has_close_reason,
    },
    types::CloseMessageReason,
    CanisterOutputCertifiedMessages, CanisterWsCloseArguments, CanisterWsCloseResult,
    CanisterWsGetMessagesArguments, CanisterWsMessageArguments, ClientKeepAliveMessageContent,
    WebsocketServiceMessageContent, CLIENT_KEEP_ALIVE_TIMEOUT_MS, CLIENT_KEEP_ALIVE_TIMEOUT_NS,
};

use super::utils::{
    actor::{
        wipe::call_wipe, ws_get_messages::call_ws_get_messages_with_panic,
        ws_message::call_ws_message_with_panic, ws_open::call_ws_open_for_client_key_with_panic,
    },
    certification::{is_message_body_valid, is_valid_certificate},
    clients::{CLIENT_1_KEY, GATEWAY_1},
    messages::{
        create_websocket_message, decode_websocket_service_message_content,
        encode_websocket_service_message_content, get_websocket_message_from_canister_message,
    },
    test_env::{get_test_env, DEFAULT_TEST_SEND_ACK_INTERVAL_MS},
};

#[test]
fn test_1_client_should_receive_ack_messages() {
    call_wipe(None);
    // open a connection for client 1
    let client_1_key = CLIENT_1_KEY.deref();
    call_ws_open_for_client_key_with_panic(client_1_key);
    // make sure there are no messages in the queue, except from first open message
    let CanisterOutputCertifiedMessages { messages, .. } = call_ws_get_messages_with_panic(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments::new(1), // skip the service open message
    );
    assert_eq!(messages.len(), 0);
    // send a message from the client in order to receive the ack with the updated sequence number
    call_ws_message_with_panic(
        &client_1_key.client_principal,
        CanisterWsMessageArguments::new(create_websocket_message(client_1_key, 1, None, false)),
    );
    // advance the canister time to make sure the ack timer expires and an ack is sent
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);

    let res =
        call_ws_get_messages_with_panic(GATEWAY_1.deref(), CanisterWsGetMessagesArguments::new(1));
    helpers::check_ack_message_result(&res, client_1_key, 1, 2);
}

#[test]
fn test_2_client_is_removed_if_keep_alive_timeout_is_reached() {
    let client_1_key = CLIENT_1_KEY.deref();
    // open a connection for client 1
    call_wipe(None);
    call_ws_open_for_client_key_with_panic(client_1_key);
    // advance the canister time to make sure the ack timer expires and an ack is sent
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);
    // get messages to check if the ack message has been set
    let msgs =
        call_ws_get_messages_with_panic(GATEWAY_1.deref(), CanisterWsGetMessagesArguments::new(1));
    helpers::check_ack_message_result(&msgs, client_1_key, 0, 2);

    // advance the canister time to make sure the keep alive timeout expires
    get_test_env().advance_canister_time_ms(CLIENT_KEEP_ALIVE_TIMEOUT_MS);

    // check if the gateway put the close message in the queue
    let msgs = call_ws_get_messages_with_panic(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments::new(1), // skip the first open message
    );
    check_canister_message_has_close_reason(
        &msgs.messages[1],
        CloseMessageReason::KeepAliveTimeout,
    );

    // the gateway should still be between the registered gateways
    // so calling the ws_close endpoint should return the ClientKeyNotConnected error
    let res = call_ws_close(
        GATEWAY_1.deref(),
        CanisterWsCloseArguments::new(client_1_key.clone()),
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
fn test_3_client_is_not_removed_if_it_sends_a_keep_alive_before_timeout() {
    let client_1_key = CLIENT_1_KEY.deref();
    call_wipe(None);
    // open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);
    // advance the canister time to make sure the ack timer expires and an ack is sent
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);
    // get messages to check if the ack message has been sent
    let res =
        call_ws_get_messages_with_panic(GATEWAY_1.deref(), CanisterWsGetMessagesArguments::new(1));
    helpers::check_ack_message_result(&res, client_1_key, 0, 2);

    let current_time = get_test_env().get_canister_time();

    // send keep alive message
    call_ws_message_with_panic(
        &client_1_key.client_principal,
        CanisterWsMessageArguments::new(create_websocket_message(
            client_1_key,
            1,
            Some(encode_websocket_service_message_content(
                &WebsocketServiceMessageContent::KeepAliveMessage(ClientKeepAliveMessageContent {
                    last_incoming_sequence_num: 1, // ignored in the CDK
                }),
            )),
            true,
        )),
    );

    // advance the canister time to make sure the keep alive timeout expires and the canister checks the keep alive,
    // keeping into account the time elapsed in the previous rounds
    let elapsed_time = get_test_env().get_canister_time() - current_time;
    get_test_env().advance_canister_time_ns(CLIENT_KEEP_ALIVE_TIMEOUT_NS - elapsed_time);
    // send a message to the canister to see the sequence number increasing in the ack message
    // and be sure that the client has not been removed
    call_ws_message_with_panic(
        &client_1_key.client_principal,
        CanisterWsMessageArguments::new(create_websocket_message(client_1_key, 2, None, false)),
    );
    // wait for the canister to send the next ack
    get_test_env()
        .advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS - CLIENT_KEEP_ALIVE_TIMEOUT_MS);
    let res = call_ws_get_messages_with_panic(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments::new(2), // skip the service open message and the fist ack message
    );
    helpers::check_ack_message_result(&res, client_1_key, 2, 3);
}

#[test]
fn test_4_client_is_not_removed_if_it_connects_while_canister_is_waiting_for_keep_alive() {
    let client_1_key = CLIENT_1_KEY.deref();
    call_wipe(None);
    // advance the canister time to make sure the ack timer expires and the canister started the keep alive timer
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);
    // open a connection for client 1
    call_ws_open_for_client_key_with_panic(client_1_key);

    // get messages for client: at this point the client doesn't expect any message
    let CanisterOutputCertifiedMessages { messages, .. } = call_ws_get_messages_with_panic(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments::new(1), // skip the service open message
    );
    assert_eq!(messages.len(), 0);

    // send a message to the canister to see the sequence number increasing in the ack message
    // and be sure that the client has not been removed
    call_ws_message_with_panic(
        &client_1_key.client_principal,
        CanisterWsMessageArguments::new(create_websocket_message(client_1_key, 1, None, false)),
    );

    // wait for the keep alive timeout to expire
    get_test_env().advance_canister_time_ms(CLIENT_KEEP_ALIVE_TIMEOUT_MS);
    // wait for the canister to send the next ack
    get_test_env()
        .advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS - CLIENT_KEEP_ALIVE_TIMEOUT_MS);

    let res = call_ws_get_messages_with_panic(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments::new(1), // skip the service open message
    );
    helpers::check_ack_message_result(&res, client_1_key, 1, 2);
}

mod helpers {
    use crate::{
        CanisterAckMessageContent, CanisterOutputCertifiedMessages, CanisterOutputMessage,
        ClientKey, WebsocketServiceMessageContent,
    };

    use super::*;

    pub(super) fn check_ack_message_result(
        res: &CanisterOutputCertifiedMessages,
        receiver_client_key: &ClientKey,
        expected_ack_sequence_number: u64,
        expected_websocket_message_sequence_number: u64,
    ) {
        let CanisterOutputCertifiedMessages {
            messages,
            cert,
            tree,
            ..
        } = res;
        assert_eq!(messages.len(), 1);
        let ack_message = messages.first().unwrap();
        check_ack_message_in_messages(
            ack_message,
            receiver_client_key,
            expected_ack_sequence_number,
            expected_websocket_message_sequence_number,
        );
        assert!(is_valid_certificate(&get_test_env(), &cert, &tree));
        assert!(is_message_body_valid(
            &ack_message.key,
            &ack_message.content,
            &tree,
        ));
    }

    fn check_ack_message_in_messages(
        ack_message: &CanisterOutputMessage,
        receiver_client_key: &ClientKey,
        expected_ack_sequence_number: u64,
        expected_websocket_message_sequence_number: u64,
    ) {
        assert_eq!(ack_message.client_key, *receiver_client_key);
        let websocket_message = get_websocket_message_from_canister_message(ack_message);
        assert_eq!(websocket_message.is_service_message, true);
        assert_eq!(
            websocket_message.sequence_num,
            expected_websocket_message_sequence_number
        );
        assert_eq!(
            websocket_message.timestamp,
            get_test_env().get_canister_time()
        );
        assert_eq!(
            decode_websocket_service_message_content(&websocket_message.content),
            WebsocketServiceMessageContent::AckMessage(CanisterAckMessageContent {
                last_incoming_sequence_num: expected_ack_sequence_number,
            }),
        );
    }
}
