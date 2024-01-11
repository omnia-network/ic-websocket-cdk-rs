use proptest::prelude::*;
use std::ops::Deref;

use crate::{
    tests::common, CanisterOutputCertifiedMessages, CanisterWsCloseArguments,
    CanisterWsGetMessagesArguments, CLIENT_KEEP_ALIVE_TIMEOUT_MS, MESSAGES_TO_DELETE_COUNT,
};

use super::utils::{
    actor::{
        send::call_send_with_panic, wipe::call_wipe, ws_close::call_ws_close_with_panic,
        ws_get_messages::call_ws_get_messages_with_panic,
        ws_open::call_ws_open_for_client_key_with_panic,
    },
    clients::{generate_random_client_key, CLIENT_1_KEY, GATEWAY_1},
    messages::{get_next_polling_nonce_from_messages, verify_messages, AppMessage},
    test_env::{
        get_test_env, DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
        DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
    },
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_1_non_registered_gateway_should_receive_empty_messages(ref test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // first, reset the canister
        call_wipe(None);

        let res = call_ws_get_messages_with_panic(
            test_gateway_principal,
            CanisterWsGetMessagesArguments::new(0),
        );
        assert_eq!(
            res,
            CanisterOutputCertifiedMessages::empty()
        );
    }

    #[test]
    fn test_2_registered_gateway_should_receive_only_open_message_if_no_messages_sent_initial_nonce(ref test_client_key in any::<u64>().prop_map(|_| generate_random_client_key())) {
        // first, reset the canister
        call_wipe(None);

        // second, register client 1
        call_ws_open_for_client_key_with_panic(test_client_key);

        let CanisterOutputCertifiedMessages { messages, .. } = call_ws_get_messages_with_panic(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments::new(0),
        );
        prop_assert_eq!(messages.len(), 1); // we expect only the service message
    }

    #[test]
    fn test_3_registered_gateway_can_receive_correct_amount_of_messages(test_send_messages_count in 1..100u64) {
        // first, reset the canister
        call_wipe(None);
        // second, register client 1
        let client_1_key = CLIENT_1_KEY.deref();
        call_ws_open_for_client_key_with_panic(client_1_key);
        // third, send a batch of messages to the client
        let messages_to_send: Vec<AppMessage> = (1..=test_send_messages_count)
            .map(|i| AppMessage {
                text: format!("test{}", i),
            })
            .collect();
        call_send_with_panic(
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
                CanisterWsGetMessagesArguments::new(i),
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
            CanisterWsGetMessagesArguments::new(messages_count),
        );
        prop_assert_eq!(
            res,
            CanisterOutputCertifiedMessages::empty()
        );
    }

    #[test]
    fn test_4_registered_gateway_can_receive_certified_messages(test_send_messages_count in 1..100u64) {
        // first, reset the canister
        call_wipe(None);
        // second, register client 1
        let client_1_key = CLIENT_1_KEY.deref();
        call_ws_open_for_client_key_with_panic(client_1_key);
        // third, send a batch of messages to the client
        let messages_to_send: Vec<AppMessage> = (1..=test_send_messages_count)
            .map(|i| AppMessage {
                text: format!("test{}", i),
            })
            .collect();
        call_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

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
                CanisterWsGetMessagesArguments::new(next_polling_nonce),
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
        call_wipe(
            Some((
                1_000, // avoid the queue size limit
                DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
            ))
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
        call_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

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
            CanisterWsGetMessagesArguments::new(0),
        );

        assert_eq!(
            messages.len(),
            // + 1 for the open service message
            // + 1 for the ack message that is never deleted because it can't be older than the send ack interval
            (test_send_messages_count + 1).saturating_sub(MESSAGES_TO_DELETE_COUNT) + 1,
        );

        let first_received_message_seq_num = if test_send_messages_count < MESSAGES_TO_DELETE_COUNT {
            // +1 for the open message
            (test_send_messages_count + 1) as u64
        } else {
            MESSAGES_TO_DELETE_COUNT as u64
        } + 1; // +1 because the seq number is incremented before sending on the canister

        // check that messages are still certified properly
        // we expect that MESSAGES_TO_DELETE_COUNT messages have already been deleted
        let mut expected_sequence_number = first_received_message_seq_num;
        let mut i = first_received_message_seq_num - 1;
        verify_messages(
            &messages,
            client_1_key,
            &cert,
            &tree,
            &mut expected_sequence_number,
            &mut i,
        );
    }
}

#[test]
fn test_6_empty_gateway_can_get_messages_until_next_keep_alive_check() {
    let send_messages_count = 10;
    // first, reset the canister
    call_wipe(None);
    // second, register client 1
    let client_1_key = CLIENT_1_KEY.deref();
    call_ws_open_for_client_key_with_panic(client_1_key);
    // third, send a batch of messages to the client
    let messages_to_send: Vec<AppMessage> = (1..=send_messages_count)
        .map(|i| AppMessage {
            text: format!("test{}", i),
        })
        .collect();
    call_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

    // check that gateway can receive the messages
    helpers::assert_gateway_has_messages(send_messages_count);

    // disconnect the client and check that gateway can still receive the messages
    call_ws_close_with_panic(
        &GATEWAY_1,
        CanisterWsCloseArguments::new(client_1_key.clone()),
    );

    // check that gateway can still receive the messages
    helpers::assert_gateway_has_messages(send_messages_count);

    // wait for the ack interval to fire
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);

    // check that gateway can still receive the messages, even after the ack interval has fired
    helpers::assert_gateway_has_messages(send_messages_count);

    // wait for the keep alive timeout to expire
    get_test_env().advance_canister_time_ms(CLIENT_KEEP_ALIVE_TIMEOUT_MS);

    helpers::assert_gateway_has_no_messages();
}

#[test]
fn test_7_empty_gateway_can_get_messages_until_next_keep_alive_check_if_removed_before_ack_interval(
) {
    let send_messages_count = 10;
    // first, reset the canister
    call_wipe(None);
    // second, register client 1
    let client_1_key = CLIENT_1_KEY.deref();
    call_ws_open_for_client_key_with_panic(client_1_key);
    // third, send a batch of messages to the client
    let messages_to_send: Vec<AppMessage> = (1..=send_messages_count)
        .map(|i| AppMessage {
            text: format!("test{}", i),
        })
        .collect();
    call_send_with_panic(&client_1_key.client_principal, messages_to_send.clone());

    // check that gateway can receive the messages
    helpers::assert_gateway_has_messages(send_messages_count);

    // wait for the ack interval to fire
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);

    // disconnect the client and check that gateway can still receive the messages
    call_ws_close_with_panic(
        &GATEWAY_1,
        CanisterWsCloseArguments::new(client_1_key.clone()),
    );

    let expected_messages_len = send_messages_count + 1; // +1 for the ack message

    // check that gateway can still receive the messages, even after the ack interval has fired
    helpers::assert_gateway_has_messages(expected_messages_len);

    // wait for the keep alive timeout to expire
    get_test_env().advance_canister_time_ms(CLIENT_KEEP_ALIVE_TIMEOUT_MS);

    // the gateway can still receive the messages, because it was emptied
    // less than an ack interval ago
    helpers::assert_gateway_has_messages(expected_messages_len);

    // wait for next ack interval to expire
    get_test_env().advance_canister_time_ms(DEFAULT_TEST_SEND_ACK_INTERVAL_MS);

    // the gateway can still receive the messages, because empty expired gateways
    // are removed only in the keep alive timeout callback
    helpers::assert_gateway_has_messages(expected_messages_len);

    // wait for the keep alive timeout to expire
    get_test_env().advance_canister_time_ms(CLIENT_KEEP_ALIVE_TIMEOUT_MS);

    helpers::assert_gateway_has_no_messages();
}

mod helpers {
    use super::*;

    pub(super) fn assert_gateway_has_messages(send_messages_count: usize) {
        let CanisterOutputCertifiedMessages { messages, .. } =
            call_ws_get_messages_with_panic(&GATEWAY_1, CanisterWsGetMessagesArguments::new(0));
        assert_eq!(
            messages.len(),
            // + 1 for the open service message
            send_messages_count + 1,
        );
    }

    pub(super) fn assert_gateway_has_no_messages() {
        let CanisterOutputCertifiedMessages { messages, .. } =
            call_ws_get_messages_with_panic(&GATEWAY_1, CanisterWsGetMessagesArguments::new(0));
        assert_eq!(messages.len(), 0);
    }
}
