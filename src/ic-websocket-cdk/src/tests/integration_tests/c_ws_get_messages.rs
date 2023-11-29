use proptest::prelude::*;
use std::ops::Deref;

use crate::{
    tests::common, CanisterOutputCertifiedMessages, CanisterWsGetMessagesArguments,
    MESSAGES_TO_DELETE_COUNT,
};

use super::utils::{
    actor::{
        ws_get_messages::call_ws_get_messages_with_panic,
        ws_open::call_ws_open_for_client_key_with_panic, ws_send::call_ws_send_with_panic,
    },
    clients::{generate_random_client_key, CLIENT_1_KEY, GATEWAY_1},
    messages::{get_next_polling_nonce_from_messages, verify_messages, AppMessage},
    test_env::{
        get_test_env, DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
        DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES, DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
    },
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_1_non_registered_gateway_should_receive_empty_messages(ref test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // first, reset the canister
        get_test_env().reset_canister_with_default_params();

        let res = call_ws_get_messages_with_panic(
            test_gateway_principal,
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        assert_eq!(
            res,
            CanisterOutputCertifiedMessages {
                messages: vec![],
                cert: vec![],
                tree: vec![],
                is_end_of_queue: true,
            },
        );
    }

    #[test]
    fn test_2_registered_gateway_should_receive_only_open_message_if_no_messages_sent_initial_nonce(ref test_client_key in any::<u64>().prop_map(|_| generate_random_client_key())) {
        // first, reset the canister
        get_test_env().reset_canister_with_default_params();

        // second, register client 1
        call_ws_open_for_client_key_with_panic(test_client_key);

        let CanisterOutputCertifiedMessages { messages, .. } = call_ws_get_messages_with_panic(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        prop_assert_eq!(messages.len(), 1); // we expect only the service message
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
            let CanisterOutputCertifiedMessages {
                messages,
                is_end_of_queue,
                ..
            } = call_ws_get_messages_with_panic(
                GATEWAY_1.deref(),
                CanisterWsGetMessagesArguments { nonce: i },
            );
            prop_assert_eq!(
                messages.len() as u64,
                if (messages_count - i) > DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES {
                    DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
                } else {
                    messages_count - i
                }
            );
            prop_assert_eq!(
                is_end_of_queue,
                (messages_count - i) <= DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES
            );
        }

        // try to get more messages than available
        let res = call_ws_get_messages_with_panic(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments {
                nonce: messages_count,
            },
        );
        prop_assert_eq!(
            res,
            CanisterOutputCertifiedMessages {
                messages: vec![],
                cert: vec![],
                tree: vec![],
                is_end_of_queue: true,
            }
        );
    }

    #[test]
    fn test_4_registered_gateway_can_receive_certified_messages(test_send_messages_count in 1..100u64) {
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
        call_ws_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

        let mut next_polling_nonce = 0;
        let mut expected_sequence_number = 1;  // `1` because the seq number is incremented before sending on the canister
        let mut i = 0;

        while next_polling_nonce <= test_send_messages_count {
            let CanisterOutputCertifiedMessages {
                messages,
                cert,
                tree,
                ..
            } = call_ws_get_messages_with_panic(
                GATEWAY_1.deref(),
                CanisterWsGetMessagesArguments {
                    nonce: next_polling_nonce,
                },
            );
            verify_messages(
                &messages,
                client_1_key,
                &cert,
                &tree,
                &mut expected_sequence_number,
                &mut i,
            );

            next_polling_nonce = get_next_polling_nonce_from_messages(messages);
        }
    }

    #[test]
    fn test_5_messages_for_gateway_are_deleted_if_old(test_send_messages_count in 1..100usize) {
        // first, reset the canister
        get_test_env().reset_canister(
            1_000, // avoid the queue size limit
            DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
            DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
        );
        // second, register client 1
        let client_1_key = CLIENT_1_KEY.deref();
        call_ws_open_for_client_key_with_panic(client_1_key);
        // third, send a batch of messages to the client
        let messages_to_send: Vec<AppMessage> = (1..=test_send_messages_count)
            .map(|i| AppMessage {
                text: format!("test{}", i),
            })
            .collect();
        call_ws_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

        // advance canister time because the messages are deleted only
        // if they're older than the send ack interval;
        // this also makes the canister send an ack message
        // and therefore delete the first batch of older messages
        get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS + 1);

        let CanisterOutputCertifiedMessages {
            messages,
            cert,
            tree,
            ..
        } = call_ws_get_messages_with_panic(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments { nonce: 0 },
        );

        assert_eq!(
            messages.len(),
            // + 1 for the open service message
            // + 1 for the ack message that is never deleted because it can't be older than the send ack interval
            (test_send_messages_count + 1).saturating_sub(MESSAGES_TO_DELETE_COUNT) + 1,
        );

        // check that messages are still certified properly
        verify_messages(
            &messages,
            client_1_key,
            &cert,
            &tree,
            // we expect that MESSAGES_TO_DELETE_COUNT messages have already been deleted
            &mut ((MESSAGES_TO_DELETE_COUNT as u64) + 1), // + 1 because the seq number is incremented before sending on the canister
            &mut( MESSAGES_TO_DELETE_COUNT as u64),
        );
    }
}
