use proptest::prelude::*;
use std::ops::Deref;

use crate::{
    tests::integration_tests::utils::clients::generate_random_client_key,
    CanisterOutputCertifiedMessages, CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult,
};

use super::utils::{
    actor::{
        ws_get_messages::call_ws_get_messages,
        ws_open::call_ws_open_for_client_key_with_panic,
        ws_send::{call_ws_send_with_panic, AppMessage},
    },
    clients::{CLIENT_1_KEY, GATEWAY_1, GATEWAY_2},
    constants::{DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES, SEND_MESSAGES_COUNT},
    messages::get_next_polling_nonce_from_messages,
    test_env::get_test_env,
};

#[test]
fn test_1_non_registered_gateway_should_receive_empty_messages() {
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();

    let res = call_ws_get_messages(
        GATEWAY_2.deref(),
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
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_2_registered_gateway_should_receive_only_open_message_if_no_messages_sent_initial_nonce(ref test_client_key in any::<u64>().prop_map(|_| generate_random_client_key())) {
        // first, reset the canister
        get_test_env().reset_canister_with_default_params();

        // second, register client 1
        call_ws_open_for_client_key_with_panic(test_client_key);

        let res = call_ws_get_messages(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        match res {
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
                prop_assert_eq!(messages.len(), 1); // we expect only the service message
            },
            _ => panic!("unexpected result"),
        };
    }

    #[test]
    fn test_3_registered_gateway_can_receive_correct_amount_of_messages(test_send_messages_count in 1..100u64) {
        // first, reset the canister
        get_test_env().reset_canister_with_default_params();
        // second, register client 1
        let client_1_key = CLIENT_1_KEY.deref();
        call_ws_open_for_client_key_with_panic(client_1_key);
        // third, send a batch of messages to the client
        let messages_to_send: Vec<AppMessage> = (1..=test_send_messages_count)
            .map(|i| AppMessage {
                text: format!("test{}", i),
            })
            .collect();
        call_ws_send_with_panic(
            &client_1_key.client_principal,
            messages_to_send.clone(),
        );

        // now we can start testing
        let messages_count = (messages_to_send.len() + 1) as u64; // +1 for the open service message
        for i in 0..messages_count {
            let res = call_ws_get_messages(
                GATEWAY_1.deref(),
                CanisterWsGetMessagesArguments { nonce: i },
            );
            match res {
                CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
                    messages, ..
                }) => {
                    prop_assert_eq!(
                        messages.len() as u64,
                        if (messages_count - i) > DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES {
                            DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
                        } else {
                            messages_count - i
                        }
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
        prop_assert_eq!(
            res,
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages {
                messages: vec![],
                cert: vec![],
                tree: vec![],
            })
        );
    }
}

#[test]
fn test_4_registered_gateway_can_receive_certified_messages() {
    // first, reset the canister
    get_test_env().reset_canister_with_default_params();
    // second, register client 1
    let client_1_key = CLIENT_1_KEY.deref();
    call_ws_open_for_client_key_with_panic(client_1_key);
    // third, send a batch of messages to the client
    let messages_to_send: Vec<AppMessage> = (1..=SEND_MESSAGES_COUNT)
        .map(|i| AppMessage {
            text: format!("test{}", i),
        })
        .collect();
    call_ws_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

    // first batch of messages
    match call_ws_get_messages(
        GATEWAY_1.deref(),
        CanisterWsGetMessagesArguments { nonce: 0 },
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

            let mut expected_sequence_number = 1; // `1` because the seq number is incremented before sending on the canister
            let mut i = 0;
            helpers::verify_messages(
                &first_batch_messages,
                client_1_key,
                &first_batch_cert,
                &first_batch_tree,
                &mut expected_sequence_number,
                &mut i,
            );

            let next_polling_nonce = get_next_polling_nonce_from_messages(first_batch_messages);
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
                        // +1 for the open service message
                        (messages_to_send.len() as u64) + 1
                            - DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES // remaining from SEND_MESSAGES_COUNT
                    );

                    helpers::verify_messages(
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

pub(crate) mod helpers {
    use crate::{
        tests::integration_tests::utils::{
            actor::ws_send::AppMessage,
            certification::{is_message_body_valid, is_valid_certificate},
            messages::decode_websocket_message,
            test_env::get_test_env,
        },
        types::WebsocketServiceMessageContent,
        CanisterOutputMessage, ClientKey,
    };
    use candid::decode_one;

    pub(crate) fn verify_messages(
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
                *expected_sequence_number,
                *index,
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
        expected_sequence_number: u64,
        index: u64,
    ) {
        assert_eq!(message.client_key, *client_key);
        let websocket_message = decode_websocket_message(&message.content);
        assert_eq!(websocket_message.client_key, *client_key);
        assert_eq!(websocket_message.sequence_num, expected_sequence_number);
        assert_eq!(
            websocket_message.timestamp,
            get_test_env().get_canister_time()
        );

        if websocket_message.is_service_message {
            let decoded_content: WebsocketServiceMessageContent =
                decode_one(&websocket_message.content).unwrap();
            assert!(
                matches!(
                    decoded_content,
                    WebsocketServiceMessageContent::AckMessage { .. }
                ) || matches!(
                    decoded_content,
                    WebsocketServiceMessageContent::OpenMessage { .. }
                )
            );
        } else {
            let decoded_content: AppMessage = decode_one(&websocket_message.content).unwrap();
            assert_eq!(
                decoded_content,
                AppMessage {
                    text: format!("test{}", index),
                }
            );
        }

        // check the certification
        assert!(is_valid_certificate(&get_test_env(), cert, tree,));
        assert!(is_message_body_valid(&message.key, &message.content, tree));
    }
}
