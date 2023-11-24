use super::common;
use crate::*;
use proptest::prelude::*;

mod utils;

#[test]
fn test_ws_handlers_are_called() {
    struct CustomState {
        is_on_open_called: bool,
        is_on_message_called: bool,
        is_on_close_called: bool,
    }

    impl CustomState {
        fn new() -> Self {
            Self {
                is_on_open_called: false,
                is_on_message_called: false,
                is_on_close_called: false,
            }
        }
    }

    thread_local! {
        static CUSTOM_STATE : RefCell<CustomState> = RefCell::new(CustomState::new());
    }

    let h = WsHandlers::default();

    set_params(WsInitParams::new(h.clone()));

    let handlers = get_handlers_from_params();

    assert!(handlers.on_open.is_none());
    assert!(handlers.on_message.is_none());
    assert!(handlers.on_close.is_none());

    handlers.call_on_open(OnOpenCallbackArgs {
        client_principal: common::generate_random_principal(),
    });
    handlers.call_on_message(OnMessageCallbackArgs {
        client_principal: common::generate_random_principal(),
        message: vec![],
    });
    handlers.call_on_close(OnCloseCallbackArgs {
        client_principal: common::generate_random_principal(),
    });

    // test that the handlers are not called if they are not initialized
    assert!(!CUSTOM_STATE.with(|h| h.borrow().is_on_open_called));
    assert!(!CUSTOM_STATE.with(|h| h.borrow().is_on_message_called));
    assert!(!CUSTOM_STATE.with(|h| h.borrow().is_on_close_called));

    // initialize handlers
    let on_open = |_| {
        CUSTOM_STATE.with(|h| {
            let mut h = h.borrow_mut();
            h.is_on_open_called = true;
        });
    };
    let on_message = |_| {
        CUSTOM_STATE.with(|h| {
            let mut h = h.borrow_mut();
            h.is_on_message_called = true;
        });
    };
    let on_close = |_| {
        CUSTOM_STATE.with(|h| {
            let mut h = h.borrow_mut();
            h.is_on_close_called = true;
        });
    };

    let h = WsHandlers {
        on_open: Some(on_open),
        on_message: Some(on_message),
        on_close: Some(on_close),
    };

    set_params(WsInitParams::new(h));

    let handlers = get_handlers_from_params();

    assert!(handlers.on_open.is_some());
    assert!(handlers.on_message.is_some());
    assert!(handlers.on_close.is_some());

    handlers.call_on_open(OnOpenCallbackArgs {
        client_principal: common::generate_random_principal(),
    });
    handlers.call_on_message(OnMessageCallbackArgs {
        client_principal: common::generate_random_principal(),
        message: vec![],
    });
    handlers.call_on_close(OnCloseCallbackArgs {
        client_principal: common::generate_random_principal(),
    });

    // test that the handlers are called if they are initialized
    assert!(CUSTOM_STATE.with(|h| h.borrow().is_on_open_called));
    assert!(CUSTOM_STATE.with(|h| h.borrow().is_on_message_called));
    assert!(CUSTOM_STATE.with(|h| h.borrow().is_on_close_called));
}

#[test]
fn test_ws_handlers_panic_is_handled() {
    let h = WsHandlers {
        on_open: Some(|_| {
            panic!("on_open_panic");
        }),
        on_message: Some(|_| {
            panic!("on_close_panic");
        }),
        on_close: Some(|_| {
            panic!("on_close_panic");
        }),
    };

    set_params(WsInitParams::new(h));

    let handlers = get_handlers_from_params();

    let res = panic::catch_unwind(|| {
        handlers.call_on_open(OnOpenCallbackArgs {
            client_principal: common::generate_random_principal(),
        });
    });
    assert!(res.is_ok());
    let res = panic::catch_unwind(|| {
        handlers.call_on_message(OnMessageCallbackArgs {
            client_principal: common::generate_random_principal(),
            message: vec![],
        });
    });
    assert!(res.is_ok());
    let res = panic::catch_unwind(|| {
        handlers.call_on_close(OnCloseCallbackArgs {
            client_principal: common::generate_random_principal(),
        });
    });
    assert!(res.is_ok());
}

#[test]
fn test_ws_init_params() {
    let handlers = WsHandlers::default();

    let params = WsInitParams::new(handlers.clone());
    assert_eq!(params.get_handlers(), handlers);
    assert_eq!(
        params.max_number_of_returned_messages,
        DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES
    );
    assert_eq!(params.send_ack_interval_ms, DEFAULT_SEND_ACK_INTERVAL_MS);
    assert_eq!(
        params.keep_alive_timeout_ms,
        DEFAULT_CLIENT_KEEP_ALIVE_TIMEOUT_MS
    );

    let params = WsInitParams::new(handlers.clone())
        .with_max_number_of_returned_messages(5)
        .with_keep_alive_timeout_ms(2)
        .with_send_ack_interval_ms(10);
    assert_eq!(params.max_number_of_returned_messages, 5);
    assert_eq!(params.keep_alive_timeout_ms, 2);
    assert_eq!(params.send_ack_interval_ms, 10);
}

#[test]
#[should_panic]
fn test_ws_init_params_keep_alive_greater() {
    WsInitParams::new(WsHandlers::default())
        .with_keep_alive_timeout_ms(10)
        .with_send_ack_interval_ms(5);
}

#[test]
#[should_panic]
fn test_ws_init_params_keep_alive_equal() {
    WsInitParams::new(WsHandlers::default())
        .with_keep_alive_timeout_ms(10)
        .with_send_ack_interval_ms(10);
}

#[test]
fn test_current_time() {
    // test
    assert_eq!(get_current_time(), 0u64);
}

proptest! {
    #[test]
    fn test_register_gateway_if_not_present(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        register_gateway_if_not_present(test_gateway_principal);

        let registered_gateway = REGISTERED_GATEWAYS.with(|map| map.borrow().get(&test_gateway_principal).cloned()).unwrap();
        prop_assert_eq!(registered_gateway, RegisteredGateway::new());

        // change the registered gateway and try to register one again
        REGISTERED_GATEWAYS.with(|map| {
            map.borrow_mut().entry(test_gateway_principal).and_modify(|gw| gw.outgoing_message_nonce = 5);
        });

        register_gateway_if_not_present(test_gateway_principal);

        let registered_gateway = REGISTERED_GATEWAYS.with(|map| map.borrow().get(&test_gateway_principal).cloned()).unwrap();
        prop_assert_eq!(registered_gateway.outgoing_message_nonce, 5);
    }

    #[test]
    fn test_get_registered_gateway(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // Set up
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(test_gateway_principal, RegisteredGateway::new()));

        let registered_gateway = get_registered_gateway(&test_gateway_principal).unwrap();
        prop_assert_eq!(registered_gateway, RegisteredGateway::new());
    }

    #[test]
    fn test_get_registered_gateway_nonexistent(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        let res = get_registered_gateway(&test_gateway_principal);
        prop_assert_eq!(
            res.err(),
            WsError::GatewayNotRegistered { gateway_principal: &test_gateway_principal }.to_string_result().err()
        );
    }

    #[test]
    fn test_is_registered_gateway(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // Set up
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(test_gateway_principal, RegisteredGateway::new()));

        let res = is_registered_gateway(&test_gateway_principal);
        prop_assert_eq!(res, true);

        let other_principal = common::generate_random_principal();
        let res = is_registered_gateway(&other_principal);
        prop_assert_eq!(res, false);
    }

    #[test]
    fn test_get_outgoing_message_nonce(test_nonce in any::<u64>(), test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // Set up
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(test_gateway_principal, RegisteredGateway { outgoing_message_nonce: test_nonce, ..Default::default() }));

        let nonce = get_outgoing_message_nonce(&test_gateway_principal).unwrap();
        prop_assert_eq!(nonce, test_nonce);
    }

    #[test]
    fn test_get_outgoing_message_nonce_nonexistent(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        let res = get_outgoing_message_nonce(&test_gateway_principal);
        prop_assert_eq!(
            res.err(),
            WsError::GatewayNotRegistered { gateway_principal: &test_gateway_principal }.to_string_result().err()
        );
    }

    #[test]
    fn test_increment_outgoing_message_nonce(test_nonce in any::<u64>()) {
        // Set up
        let gateway_principal = common::generate_random_principal();
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(gateway_principal, RegisteredGateway { outgoing_message_nonce: test_nonce, ..Default::default() }));

        let res = increment_outgoing_message_nonce(&gateway_principal);
        prop_assert!(res.is_ok());

        let nonce = REGISTERED_GATEWAYS.with(|n| n.borrow().get(&gateway_principal).unwrap().outgoing_message_nonce);
        prop_assert_eq!(nonce, test_nonce + 1);
    }

    #[test]
    fn test_increment_outgoing_message_nonce_nonexistent(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // Set up
        let res = increment_outgoing_message_nonce(&test_gateway_principal);
        prop_assert_eq!(
            res.err(),
            WsError::GatewayNotRegistered { gateway_principal: &test_gateway_principal }.to_string_result().err()
        )
    }

    #[test]
    fn test_insert_client(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        // Set up
        let registered_client = utils::generate_random_registered_client();

        insert_client(test_client_key.clone(), registered_client.clone());

        let actual_client_key = CURRENT_CLIENT_KEY_MAP.with(|map| map.borrow().get(&test_client_key.client_principal).cloned()).unwrap();
        prop_assert_eq!(actual_client_key, test_client_key.clone());

        let actual_client = REGISTERED_CLIENTS.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(actual_client, registered_client);
    }

    #[test]
    fn test_get_registered_client(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        // Set up
        let test_client = utils::generate_random_registered_client();
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_client.clone());
        });

        let client = get_registered_client(&test_client_key).unwrap();
        prop_assert_eq!(client, test_client);
    }

    #[test]
    fn test_get_registered_client_nonexistent(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        let res = get_registered_client(&test_client_key);
        prop_assert_eq!(
            res.err(),
            WsError::ClientKeyNotConnected { client_key: &test_client_key }.to_string_result().err()
        );
    }

    #[test]
    fn test_get_client_key_from_principal(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        // Set up
        CURRENT_CLIENT_KEY_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.client_principal, test_client_key.clone());
        });

        let client_key = get_client_key_from_principal(&test_client_key.client_principal).unwrap();
        prop_assert_eq!(client_key, test_client_key);
    }

    #[test]
    fn test_get_client_key_from_principal_empty(test_client_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        let res = get_client_key_from_principal(&test_client_principal);
        prop_assert_eq!(
            res.err(),
            WsError::ClientPrincipalNotConnected { client_principal: &test_client_principal }.to_string_result().err()
        );
    }

    #[test]
    fn test_check_registered_client_exists(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        // Set up
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), utils::generate_random_registered_client());
        });

        let res = check_registered_client_exists(&test_client_key);
        prop_assert!(res.is_ok());
    }

    #[test]
    fn test_check_registered_client_exists_empty(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        let res = check_registered_client_exists(&test_client_key);
        prop_assert_eq!(
            res.err(),
            WsError::ClientKeyNotConnected { client_key: &test_client_key }.to_string_result().err()
        );
    }

    #[test]
    fn test_check_client_registered_to_gateway(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key()), test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // Set up
        let registered_client = RegisteredClient::new(test_gateway_principal);
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), registered_client);
        });

        let res = check_client_registered_to_gateway(&test_client_key, &test_gateway_principal);
        prop_assert!(res.is_ok());
    }

    #[test]
    fn test_check_client_registered_to_gateway_empty(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        let test_gateway_principal = common::generate_random_principal();
        let res = check_client_registered_to_gateway(&test_client_key, &test_gateway_principal);
        prop_assert_eq!(
            res.err(),
            WsError::ClientKeyNotConnected { client_key: &test_client_key }.to_string_result().err()
        );
    }

    #[test]
    fn test_check_client_registered_to_gateway_wrong_gateway(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key()), test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal())) {
        // Set up
        let registered_client = RegisteredClient::new(test_gateway_principal);
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), registered_client);
        });
        let wrong_gateway_principal = common::generate_random_principal();
        let res = check_client_registered_to_gateway(&test_client_key, &wrong_gateway_principal);
        prop_assert_eq!(
            res.err(),
            WsError::ClientNotRegisteredToGateway { client_key: &test_client_key, gateway_principal: &wrong_gateway_principal }.to_string_result().err()
        );
    }

    #[test]
    fn test_add_client_to_wait_for_keep_alive(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        add_client_to_wait_for_keep_alive(&test_client_key);

        let is_client_in_map = CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|map| map.borrow().get(&test_client_key).is_some());
        prop_assert_eq!(is_client_in_map, true);
    }

    #[test]
    fn test_init_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        init_outgoing_message_to_client_num(test_client_key.clone());

        let seq_num = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(seq_num, INITIAL_CANISTER_SEQUENCE_NUM);
    }

    #[test]
    fn test_get_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let num = get_outgoing_message_to_client_num(&test_client_key).unwrap();
        prop_assert_eq!(num, test_num);
    }

    #[test]
    fn test_get_outgoing_message_to_client_num_nonexistent(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        let res = get_outgoing_message_to_client_num(&test_client_key);
        prop_assert_eq!(
            res.err(),
            WsError::OutgoingMessageToClientNumNotInitialized { client_key: &test_client_key }.to_string_result().err()
        );
    }

    #[test]
    fn test_increment_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let res = increment_outgoing_message_to_client_num(&test_client_key);
        prop_assert!(res.is_ok());

        let num = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(num, test_num + 1);
    }

    #[test]
    fn test_init_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        init_expected_incoming_message_from_client_num(test_client_key.clone());

        let seq_num = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(seq_num, INITIAL_CLIENT_SEQUENCE_NUM);
    }

    #[test]
    fn test_get_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let num = get_expected_incoming_message_from_client_num(&test_client_key).unwrap();
        prop_assert_eq!(num, test_num);
    }

    #[test]
    fn test_get_expected_incoming_message_from_client_num_nonexistent(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        let res = get_expected_incoming_message_from_client_num(&test_client_key);
        prop_assert_eq!(
            res.err(),
            WsError::ExpectedIncomingMessageToClientNumNotInitialized { client_key: &test_client_key }.to_string_result().err()
        );
    }

    #[test]
    fn test_increment_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let res = increment_expected_incoming_message_from_client_num(&test_client_key);
        prop_assert!(res.is_ok());

        let num = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(num, test_num + 1);
    }

    #[test]
    fn test_add_client(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        let test_registered_client = utils::generate_random_registered_client();

        // Test
        add_client(test_client_key.clone(), test_registered_client.clone());

        let client_key = CURRENT_CLIENT_KEY_MAP.with(|map| map.borrow().get(&test_client_key.client_principal).cloned()).unwrap();
        prop_assert_eq!(client_key, test_client_key.clone());

        let registered_client = REGISTERED_CLIENTS.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(registered_client, test_registered_client);

        let client_seq_num = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(client_seq_num, INITIAL_CLIENT_SEQUENCE_NUM);

        let canister_seq_num = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned()).unwrap();
        prop_assert_eq!(canister_seq_num, INITIAL_CANISTER_SEQUENCE_NUM);
    }

    #[test]
    fn test_remove_client(test_client_key in any::<u8>().prop_map(|_| common::get_random_client_key())) {
        // Set up
        CURRENT_CLIENT_KEY_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.client_principal.clone(), test_client_key.clone());
        });
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), utils::generate_random_registered_client());
        });
        INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), INITIAL_CLIENT_SEQUENCE_NUM);
        });
        OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), INITIAL_CANISTER_SEQUENCE_NUM);
        });

        remove_client(&test_client_key);

        let client_key = CURRENT_CLIENT_KEY_MAP.with(|map| map.borrow().get(&test_client_key.client_principal).cloned());
        prop_assert!(client_key.is_none());

        let registered_client = REGISTERED_CLIENTS.with(|map| map.borrow().get(&test_client_key).cloned());
        prop_assert!(registered_client.is_none());

        let incoming_seq_num = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned());
        prop_assert!(incoming_seq_num.is_none());

        let outgoing_seq_num = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).cloned());
        prop_assert!(outgoing_seq_num.is_none());
    }

    #[test]
    fn test_format_message_for_gateway_key(test_gateway_principal in any::<u8>().prop_map(|_| common::generate_random_principal()), test_nonce in any::<u64>()) {
        let message_key = format_message_for_gateway_key(&test_gateway_principal, test_nonce);
        prop_assert_eq!(message_key, test_gateway_principal.to_string() + "_" + &format!("{:0>20}", test_nonce.to_string()));
    }

    #[test]
    fn test_get_messages_for_gateway_range_empty(messages_count in 0..1000u64) {
        // Set up
        utils::initialize_params();
        let gateway_principal = common::generate_random_principal();
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(gateway_principal, RegisteredGateway::new()));

        // Test
        // we ask for a random range of messages to check if it always returns the same range for empty messages
        for i in 0..messages_count {
            let (start_index, end_index) = get_messages_for_gateway_range(&gateway_principal, i);
            prop_assert_eq!(start_index, 0);
            prop_assert_eq!(end_index, 0);
        }
    }

    #[test]
    fn test_get_messages_for_gateway_range_smaller_than_max(gateway_principal in any::<u8>().prop_map(|_| common::get_static_principal())) {
        // Set up
        utils::initialize_params();
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(gateway_principal, RegisteredGateway::new()));

        let messages_count = 4;
        let test_client_key = common::get_random_client_key();
        utils::add_messages_for_gateway(test_client_key, &gateway_principal, messages_count);

        // Test
        // messages are just 4, so we don't exceed the max number of returned messages
        // add one to test the out of range index
        for i in 0..messages_count + 1 {
            let (start_index, end_index) = get_messages_for_gateway_range(&gateway_principal, i);
            prop_assert_eq!(start_index, i as usize);
            prop_assert_eq!(end_index, messages_count as usize);
        }

        // Clean up
        utils::clean_messages_for_gateway(&gateway_principal);
    }

    #[test]
    fn test_get_messages_for_gateway_range_larger_than_max(gateway_principal in any::<u8>().prop_map(|_| common::get_static_principal()), max_number_of_returned_messages in 0..1000usize) {
        // Set up
        set_params(WsInitParams {
            max_number_of_returned_messages,
            ..Default::default()
        });
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(gateway_principal, RegisteredGateway::new()));

        let messages_count: u64 = (2 * max_number_of_returned_messages).try_into().unwrap();
        let test_client_key = common::get_random_client_key();
        utils::add_messages_for_gateway(test_client_key, &gateway_principal, messages_count);

        // Test
        // messages are now 2 * MAX_NUMBER_OF_RETURNED_MESSAGES
        // the case in which the start index is 0 is tested in test_get_messages_for_gateway_range_initial_nonce
        for i in 1..messages_count + 1 {
            let (start_index, end_index) = get_messages_for_gateway_range(&gateway_principal, i);
            let expected_end_index = if (i as usize) + max_number_of_returned_messages > messages_count as usize {
                messages_count as usize
            } else {
                (i as usize) + max_number_of_returned_messages
            };
            prop_assert_eq!(start_index, i as usize);
            prop_assert_eq!(end_index, expected_end_index);
        }

        // Clean up
        utils::clean_messages_for_gateway(&gateway_principal);
    }

    #[test]
    fn test_get_messages_for_gateway_initial_nonce(gateway_principal in any::<u8>().prop_map(|_| common::get_static_principal()), messages_count in 0..100u64, max_number_of_returned_messages in 0..1000usize) {
        // Set up
        set_params(WsInitParams {
            max_number_of_returned_messages,
            ..Default::default()
        });
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(gateway_principal, RegisteredGateway::new()));

        let test_client_key = common::get_random_client_key();
        utils::add_messages_for_gateway(test_client_key, &gateway_principal, messages_count);

        // Test
        let (start_index, end_index) = get_messages_for_gateway_range(&gateway_principal, 0);
        let expected_start_index = if (messages_count as usize) > max_number_of_returned_messages {
            (messages_count as usize) - max_number_of_returned_messages
        } else {
            0
        };
        prop_assert_eq!(start_index, expected_start_index);
        prop_assert_eq!(end_index, messages_count as usize);

        // Clean up
        utils::clean_messages_for_gateway(&gateway_principal);
    }

    #[test]
    fn test_get_messages_for_gateway(gateway_principal in any::<u8>().prop_map(|_| common::get_static_principal()), messages_count in 0..100u64) {
        // Set up
        REGISTERED_GATEWAYS.with(|n| n.borrow_mut().insert(gateway_principal, RegisteredGateway::new()));

        let test_client_key = common::get_random_client_key();
        utils::add_messages_for_gateway(test_client_key, &gateway_principal, messages_count);

        // Test
        // add one to test the out of range index
        for i in 0..messages_count + 1 {
            let (start_index, end_index) = get_messages_for_gateway_range(&gateway_principal, i);
            let messages = get_messages_for_gateway(&gateway_principal, start_index, end_index);

            // check if the messages returned are the ones we expect
            for (j, message) in messages.iter().enumerate() {
                let expected_key = format_message_for_gateway_key(&gateway_principal, (start_index + j) as u64);
                prop_assert_eq!(&message.key, &expected_key);
            }
        }

        // Clean up
        utils::clean_messages_for_gateway(&gateway_principal);
    }

    #[test]
    fn test_serialize_websocket_message(test_msg_bytes in any::<Vec<u8>>(), test_sequence_num in any::<u64>(), test_timestamp in any::<u64>()) {
        // TODO: add more tests, in which we check the serialized message
        let websocket_message = WebsocketMessage {
            client_key: common::get_random_client_key(),
            sequence_num: test_sequence_num,
            timestamp: test_timestamp,
            is_service_message: false,
            content: test_msg_bytes,
        };

        let serialized_message = websocket_message.cbor_serialize();
        prop_assert!(serialized_message.is_ok()); // not so useful as a test
    }
}
