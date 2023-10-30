use std::ops::Deref;

use ic_websocket_cdk::{
    CanisterOutputCertifiedMessages, CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult,
};

use crate::{
    actor::{
        ws_get_messages::call_ws_get_messages,
        ws_open::call_ws_open_for_client_key_with_panic,
        ws_send::{call_ws_send_with_panic, AppMessage},
    },
    clients::{CLIENT_1_KEY, GATEWAY_1, GATEWAY_2},
    constants::{DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES, SEND_MESSAGES_COUNT},
    messages::get_next_polling_nonce_from_messages,
    TEST_ENV,
};

#[test]
fn test_1_fails_if_a_non_registered_gateway_tries_to_get_messages() {
    // first, reset the canister
    TEST_ENV.reset_canister_with_default_params();

    let res = call_ws_get_messages(
        GATEWAY_2.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );
    assert_eq!(
        res,
        CanisterWsGetMessagesResult::Err(String::from(
            "caller is not the gateway that has been registered during CDK initialization",
        )),
    );
}

#[test]
fn test_2_registered_gateway_should_receive_empty_messages_if_no_messages_are_available() {
    let res = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
    );
    assert_eq!(
        res,
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
            messages: vec![],
            cert: vec![],
            tree: vec![],
        }),
    );

    // test also with a high nonce to make sure the indexes are calculated correctly in the canister
    let res = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 100 },
    );
    assert_eq!(
        res,
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
            messages: vec![],
            cert: vec![],
            tree: vec![],
        }),
    );
}

#[test]
fn test_3_registered_gateway_can_receive_correct_amount_of_messages() {
    // first, register client 1
    let client_1_key = CLIENT_1_KEY.deref();
    call_ws_open_for_client_key_with_panic(client_1_key);
    // second, send a batch of messages to the client
    call_ws_send_with_panic(
        &client_1_key.client_principal,
        (0..SEND_MESSAGES_COUNT)
            .map(|i| AppMessage {
                text: format!("test{}", i),
            })
            .collect(),
    );

    // now we can start testing
    let messages_count = SEND_MESSAGES_COUNT + 1; // +1 for the open service message
    for i in 0..messages_count {
        let res = call_ws_get_messages(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments { nonce: i },
        );
        match res {
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
                messages, ..
            }) => {
                assert_eq!(
                    messages.len() as u64,
                    if messages_count - i > DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES {
                        DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
                    } else {
                        messages_count - i
                    },
                );
            },
            _ => panic!("unexpected result"),
        };
    }

    // try to get more messages than available
    let res = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments {
            nonce: messages_count,
        },
    );
    match res {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
            assert_eq!(messages.len(), 0);
        },
        _ => panic!("unexpected result"),
    };
}

#[test]
fn test_4_registered_gateway_can_receive_certified_messages() {
    let client_1_key = CLIENT_1_KEY.deref();

    // first batch of messages
    match call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 1 }, // skip the case in which the gateway restarts polling from the beginning (tested below)
    ) {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
            messages: first_batch_messages,
            cert: first_batch_cert,
            tree: first_batch_tree,
        }) => {
            assert_eq!(
                first_batch_messages.len() as u64,
                DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
            );

            let mut expected_sequence_number = 2; // first is the service open message and the number is incremented before sending
            let mut i = 0;
            utils::verify_messages(
                &first_batch_messages,
                client_1_key,
                &first_batch_cert,
                &first_batch_tree,
                &mut expected_sequence_number,
                &mut i,
            );

            let next_polling_nonce = get_next_polling_nonce_from_messages(first_batch_messages);
            println!("next polling nonce: {}", next_polling_nonce);
            // second batch of messages
            match call_ws_get_messages(
                GATEWAY_1.deref(),
                CanisterWsGetMessagesArguments {
                    nonce: next_polling_nonce,
                },
            ) {
                CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
                    messages: second_batch_messages,
                    cert: second_batch_cert,
                    tree: second_batch_tree,
                }) => {
                    assert_eq!(
                        second_batch_messages.len() as u64,
                        SEND_MESSAGES_COUNT - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES // remaining from SEND_MESSAGES_COUNT
                    );

                    utils::verify_messages(
                        &second_batch_messages,
                        client_1_key,
                        &second_batch_cert,
                        &second_batch_tree,
                        &mut expected_sequence_number,
                        &mut i,
                    );
                },
                _ => panic!("unexpected result"),
            }
        },
        _ => panic!("unexpected result"),
    }
}

#[test]
fn test_5_registered_gateway_can_poll_messages_after_restart() {
    let res = call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 }, // start polling from the beginning, as if the gateway restarted
    );
    match res {
        CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
            messages,
            cert,
            tree,
        }) => {
            // +1 for the service open message +1 because the seq num is incremented before sending
            let mut expected_sequence_number =
                SEND_MESSAGES_COUNT - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES + 1 + 1;
            let mut i = SEND_MESSAGES_COUNT - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES;

            utils::verify_messages(
                &messages,
                CLIENT_1_KEY.deref(),
                &cert,
                &tree,
                &mut expected_sequence_number,
                &mut i,
            );
        },
        _ => panic!("unexpected result"),
    };
}

mod utils {
    use candid::decode_one;
    use ic_websocket_cdk::{CanisterOutputMessage, ClientKey};

    use crate::{
        actor::ws_send::AppMessage,
        certification::{is_message_body_valid, is_valid_certificate},
        messages::decode_websocket_message,
        TEST_ENV,
    };

    pub fn verify_messages(
        messages: &Vec<CanisterOutputMessage>,
        client_key: &ClientKey,
        cert: &[u8],
        tree: &[u8],
        expected_sequence_number: &mut u64,
        index: &mut u64,
    ) {
        for message in messages.iter() {
            verify_message(
                message,
                client_key,
                cert,
                tree,
                expected_sequence_number,
                index,
            );

            *expected_sequence_number += 1;
            *index += 1;
        }
    }

    fn verify_message(
        message: &CanisterOutputMessage,
        client_key: &ClientKey,
        cert: &[u8],
        tree: &[u8],
        expected_sequence_number: &u64,
        index: &u64,
    ) {
        assert_eq!(message.client_key, *client_key);
        let websocket_message = decode_websocket_message(&message.content);
        assert_eq!(websocket_message.client_key, *client_key);
        assert_eq!(websocket_message.sequence_num, *expected_sequence_number);
        assert_eq!(websocket_message.timestamp, TEST_ENV.get_canister_time());
        assert_eq!(websocket_message.is_service_message, false);
        let decoded_content: AppMessage = decode_one(&websocket_message.content).unwrap();
        assert_eq!(
            decoded_content,
            AppMessage {
                text: format!("test{}", index)
            }
        );

        // check the certification
        assert!(is_valid_certificate(
            TEST_ENV.canister_id,
            cert,
            tree,
            &TEST_ENV.get_root_ic_key()
        ));
        assert!(is_message_body_valid(&message.key, &message.content, tree));
    }
}
