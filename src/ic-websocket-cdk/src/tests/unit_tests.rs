use super::super::*;
use proptest::prelude::*;

mod test_utils {
    use candid::Principal;
    use ic_agent::{identity::BasicIdentity, Identity};
    use ring::signature::Ed25519KeyPair;

    use super::{
        get_message_for_gateway_key, CanisterOutputMessage, ClientKey, RegisteredClient,
        MESSAGES_FOR_GATEWAYS,
    };

    fn generate_random_key_pair() -> Ed25519KeyPair {
        let rng = ring::rand::SystemRandom::new();
        let key_pair =
            Ed25519KeyPair::generate_pkcs8(&rng).expect("Could not generate a key pair.");
        Ed25519KeyPair::from_pkcs8(key_pair.as_ref()).expect("Could not read the key pair.")
    }

    pub fn generate_random_principal() -> candid::Principal {
        let key_pair = generate_random_key_pair();
        let identity = BasicIdentity::from_key_pair(key_pair);

        // workaround to keep the principal in the version of candid used by the canister
        candid::Principal::from_text(identity.sender().unwrap().to_text()).unwrap()
    }

    pub(super) fn generate_random_registered_client() -> RegisteredClient {
        RegisteredClient::new(Principal::anonymous())
    }

    pub fn get_static_principal() -> Principal {
        Principal::from_text("wnkwv-wdqb5-7wlzr-azfpw-5e5n5-dyxrf-uug7x-qxb55-mkmpa-5jqik-tqe")
            .unwrap() // a random static but valid principal
    }

    pub(super) fn get_random_client_key() -> ClientKey {
        ClientKey::new(
            generate_random_principal(),
            // a random nonce
            rand::random(),
        )
    }

    pub(super) fn add_messages_for_gateway(
        client_key: ClientKey,
        gateway_principal: Principal,
        count: u64,
    ) {
        MESSAGES_FOR_GATEWAYS.with(|m| {
            for i in 0..count {
                m.borrow_mut()
                    .get_mut(&gateway_principal)
                    .expect("TODO")
                    .push_back(CanisterOutputMessage {
                        client_key: client_key.clone(),
                        key: get_message_for_gateway_key(gateway_principal.clone(), i),
                        content: vec![],
                    });
            }
        });
    }

    pub fn clean_messages_for_gateway() {
        MESSAGES_FOR_GATEWAYS.with(|m| m.borrow_mut().clear());
    }
}

// we don't need to proptest get_gateway_principal if principal is not set, as it just panics
#[test]
#[should_panic = "gateway should be initialized"]
fn test_get_gateway_principal_not_set() {
    get_registered_gateways_principals();
}

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

    let mut h = WsHandlers {
        on_open: None,
        on_message: None,
        on_close: None,
    };

    set_params(WsInitParams {
        handlers: h.clone(),
        ..Default::default()
    });

    let handlers = get_handlers_from_params();

    assert!(handlers.on_open.is_none());
    assert!(handlers.on_message.is_none());
    assert!(handlers.on_close.is_none());

    handlers.call_on_open(OnOpenCallbackArgs {
        client_principal: test_utils::generate_random_principal(),
    });
    handlers.call_on_message(OnMessageCallbackArgs {
        client_principal: test_utils::generate_random_principal(),
        message: vec![],
    });
    handlers.call_on_close(OnCloseCallbackArgs {
        client_principal: test_utils::generate_random_principal(),
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

    h = WsHandlers {
        on_open: Some(on_open),
        on_message: Some(on_message),
        on_close: Some(on_close),
    };

    set_params(WsInitParams {
        handlers: h.clone(),
        ..Default::default()
    });

    let handlers = get_handlers_from_params();

    assert!(handlers.on_open.is_some());
    assert!(handlers.on_message.is_some());
    assert!(handlers.on_close.is_some());

    handlers.call_on_open(OnOpenCallbackArgs {
        client_principal: test_utils::generate_random_principal(),
    });
    handlers.call_on_message(OnMessageCallbackArgs {
        client_principal: test_utils::generate_random_principal(),
        message: vec![],
    });
    handlers.call_on_close(OnCloseCallbackArgs {
        client_principal: test_utils::generate_random_principal(),
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

    set_params(WsInitParams {
        handlers: h.clone(),
        ..Default::default()
    });

    let handlers = get_handlers_from_params();

    let res = panic::catch_unwind(|| {
        handlers.call_on_open(OnOpenCallbackArgs {
            client_principal: test_utils::generate_random_principal(),
        });
    });
    assert!(res.is_ok());
    let res = panic::catch_unwind(|| {
        handlers.call_on_message(OnMessageCallbackArgs {
            client_principal: test_utils::generate_random_principal(),
            message: vec![],
        });
    });
    assert!(res.is_ok());
    let res = panic::catch_unwind(|| {
        handlers.call_on_close(OnCloseCallbackArgs {
            client_principal: test_utils::generate_random_principal(),
        });
    });
    assert!(res.is_ok());
}

#[test]
fn test_current_time() {
    // test
    assert_eq!(get_current_time(), 0u64);
}

proptest! {
    #[test]
    fn test_initialize_registered_gateway(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
        initialize_registered_gateways(vec![test_gateway_principal.to_string()]);

        REGISTERED_GATEWAYS.with(|p| {
            let p = p.borrow();
            assert!(p.is_some());
            assert_eq!(
                p.to_owned().unwrap()[0],
                RegisteredGateway::new(test_gateway_principal)
            );
        });
    }

    #[test]
    fn test_get_outgoing_message_nonce(test_nonce in any::<u64>()) {
        // Set up
        OUTGOING_MESSAGE_NONCE.with(|n| *n.borrow_mut() = test_nonce);

        let actual_nonce = get_outgoing_message_nonce();
        prop_assert_eq!(actual_nonce, test_nonce);
    }

    #[test]
    fn test_increment_outgoing_message_nonce(test_nonce in any::<u64>()) {
        // Set up
        OUTGOING_MESSAGE_NONCE.with(|n| *n.borrow_mut() = test_nonce);

        increment_outgoing_message_nonce();
        prop_assert_eq!(get_outgoing_message_nonce(), test_nonce + 1);
    }

    #[test]
    fn test_insert_client(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        // Set up
        let registered_client = test_utils::generate_random_registered_client();

        insert_client(test_client_key.clone(), registered_client.clone());

        let actual_client_key = CURRENT_CLIENT_KEY_MAP.with(|map| map.borrow().get(&test_client_key.client_principal).unwrap().clone());
        prop_assert_eq!(actual_client_key, test_client_key.clone());

        let actual_client = REGISTERED_CLIENTS.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_client, registered_client);
    }

    #[test]
    fn test_get_gateway_principal(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
        // Set up
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(test_gateway_principal.clone())]));

        let actual_gateway_principal = get_registered_gateways_principals()[0];
        prop_assert_eq!(actual_gateway_principal, test_gateway_principal);
    }

    #[test]
    fn test_is_client_registered_empty(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        let actual_result = is_client_registered(&test_client_key);
        prop_assert_eq!(actual_result, false);
    }

    #[test]
    fn test_is_client_registered(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        // Set up
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_utils::generate_random_registered_client());
        });

        let actual_result = is_client_registered(&test_client_key);
        prop_assert_eq!(actual_result, true);
    }

    #[test]
    fn test_get_client_key_from_principal_empty(test_client_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
        let actual_result = get_client_key_from_principal(&test_client_principal);
        prop_assert_eq!(actual_result.err(), Some(String::from(format!(
            "client with principal {} doesn't have an open connection",
            test_client_principal
        ))));
    }

    #[test]
    fn test_get_client_key_from_principal(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        // Set up
        CURRENT_CLIENT_KEY_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.client_principal, test_client_key.clone());
        });

        let actual_result = get_client_key_from_principal(&test_client_key.client_principal);
        prop_assert_eq!(actual_result.unwrap(), test_client_key);
    }

    #[test]
    fn test_check_registered_client_empty(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        let actual_result = check_registered_client(&test_client_key);
        prop_assert_eq!(actual_result.err(), Some(format!("client with key {} doesn't have an open connection", test_client_key)));
    }

    #[test]
    fn test_check_registered_client(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        // Set up
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_utils::generate_random_registered_client());
        });

        let actual_result = check_registered_client(&test_client_key);
        prop_assert!(actual_result.is_ok());
        let non_existing_client_key = test_utils::get_random_client_key();
        let actual_result = check_registered_client(&non_existing_client_key);
        prop_assert_eq!(actual_result.err(), Some(format!("client with key {} doesn't have an open connection", non_existing_client_key)));
    }

    #[test]
    fn test_init_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        init_outgoing_message_to_client_num(test_client_key.clone());

        let actual_result = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, INITIAL_CANISTER_SEQUENCE_NUM);
    }

    #[test]
    fn test_increment_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let increment_result = increment_outgoing_message_to_client_num(&test_client_key);
        prop_assert!(increment_result.is_ok());

        let actual_result = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, test_num + 1);
    }

    #[test]
    fn test_get_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let actual_result = get_outgoing_message_to_client_num(&test_client_key);
        prop_assert!(actual_result.is_ok());
        prop_assert_eq!(actual_result.unwrap(), test_num);
    }

    #[test]
    fn test_init_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        init_expected_incoming_message_from_client_num(test_client_key.clone());

        let actual_result = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, INITIAL_CLIENT_SEQUENCE_NUM);
    }

    #[test]
    fn test_get_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let actual_result = get_expected_incoming_message_from_client_num(&test_client_key);
        prop_assert!(actual_result.is_ok());
        prop_assert_eq!(actual_result.unwrap(), test_num);
    }

    #[test]
    fn test_increment_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key()), test_num in any::<u64>()) {
        // Set up
        INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_num);
        });

        let increment_result = increment_expected_incoming_message_from_client_num(&test_client_key);
        prop_assert!(increment_result.is_ok());

        let actual_result = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, test_num + 1);
    }

    #[test]
    fn test_add_client_to_wait_for_keep_alive(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        add_client_to_wait_for_keep_alive(&test_client_key);

        let actual_result = CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|map| map.borrow().get(&test_client_key).is_some());
        prop_assert_eq!(actual_result, true);
    }

    #[test]
    fn test_add_client(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        let registered_client = test_utils::generate_random_registered_client();

        // Test
        add_client(test_client_key.clone(), registered_client.clone());

        let actual_result = CURRENT_CLIENT_KEY_MAP.with(|map| map.borrow().get(&test_client_key.client_principal).unwrap().clone());
        prop_assert_eq!(actual_result, test_client_key.clone());

        let actual_result = REGISTERED_CLIENTS.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, registered_client);

        let actual_result = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, INITIAL_CLIENT_SEQUENCE_NUM);

        let actual_result = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
        prop_assert_eq!(actual_result, INITIAL_CANISTER_SEQUENCE_NUM);
    }

    #[test]
    fn test_remove_client(test_client_key in any::<u8>().prop_map(|_| test_utils::get_random_client_key())) {
        // Set up
        CURRENT_CLIENT_KEY_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.client_principal.clone(), test_client_key.clone());
        });
        REGISTERED_CLIENTS.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), test_utils::generate_random_registered_client());
        });
        INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), INITIAL_CLIENT_SEQUENCE_NUM);
        });
        OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
            map.borrow_mut().insert(test_client_key.clone(), INITIAL_CANISTER_SEQUENCE_NUM);
        });

        remove_client(&test_client_key);

        let is_none = CURRENT_CLIENT_KEY_MAP.with(|map| map.borrow().get(&test_client_key.client_principal).is_none());
        prop_assert!(is_none);

        let is_none = REGISTERED_CLIENTS.with(|map| map.borrow().get(&test_client_key).is_none());
        prop_assert!(is_none);

        let is_none = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).is_none());
        prop_assert!(is_none);

        let is_none = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).is_none());
        prop_assert!(is_none);
    }

    #[test]
    fn test_get_message_for_gateway_key(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal()), test_nonce in any::<u64>()) {
        let actual_result = get_message_for_gateway_key(test_gateway_principal.clone(), test_nonce);
        prop_assert_eq!(actual_result, test_gateway_principal.to_string() + "_" + &format!("{:0>20}", test_nonce.to_string()));
    }

    #[test]
    fn test_get_messages_for_gateway_range_empty(messages_count in any::<u64>().prop_map(|c| c % 1000)) {
        // Set up
        let gateway_principal = test_utils::generate_random_principal();
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(gateway_principal.clone())]));
        MESSAGES_FOR_GATEWAYS.with(|m| *m.borrow_mut() = {
            let mut m = HashMap::new();
            m.insert(gateway_principal.clone(), VecDeque::new());
            m
        });

        // Test
        // we ask for a random range of messages to check if it always returns the same range for empty messages
        for i in 0..messages_count {
            let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
            prop_assert_eq!(start_index, 0);
            prop_assert_eq!(end_index, 0);
        }
    }

    #[test]
    fn test_get_messages_for_gateway_range_smaller_than_max(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal())) {
        // Set up
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(gateway_principal.clone())]));
        MESSAGES_FOR_GATEWAYS.with(|m| *m.borrow_mut() = {
            let mut m = HashMap::new();
            m.insert(gateway_principal.clone(), VecDeque::new());
            m
        });

        let messages_count = 4;
        let test_client_key = test_utils::get_random_client_key();
        test_utils::add_messages_for_gateway(test_client_key, gateway_principal, messages_count);

        // Test
        // messages are just 4, so we don't exceed the max number of returned messages
        // add one to test the out of range index
        for i in 0..messages_count + 1 {
            let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
            prop_assert_eq!(start_index, i as usize);
            prop_assert_eq!(end_index, messages_count as usize);
        }

        // Clean up
        test_utils::clean_messages_for_gateway();
    }

    #[test]
    fn test_get_messages_for_gateway_range_larger_than_max(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal()), max_number_of_returned_messages in any::<usize>().prop_map(|c| c % 1000)) {
        // Set up
        PARAMS.with(|p| {
            *p.borrow_mut() = WsInitParams {
                max_number_of_returned_messages,
                ..Default::default()
            }
        });
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(gateway_principal.clone())]));
        MESSAGES_FOR_GATEWAYS.with(|m| *m.borrow_mut() = {
            let mut m = HashMap::new();
            m.insert(gateway_principal.clone(), VecDeque::new());
            m
        });

        let messages_count: u64 = (2 * max_number_of_returned_messages).try_into().unwrap();
        let test_client_key = test_utils::get_random_client_key();
        test_utils::add_messages_for_gateway(test_client_key, gateway_principal, messages_count);

        // Test
        // messages are now 2 * MAX_NUMBER_OF_RETURNED_MESSAGES
        // the case in which the start index is 0 is tested in test_get_messages_for_gateway_range_initial_nonce
        for i in 1..messages_count + 1 {
            let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
            let expected_end_index = if (i as usize) + max_number_of_returned_messages > messages_count as usize {
                messages_count as usize
            } else {
                (i as usize) + max_number_of_returned_messages
            };
            prop_assert_eq!(start_index, i as usize);
            prop_assert_eq!(end_index, expected_end_index);
        }

        // Clean up
        test_utils::clean_messages_for_gateway();
    }

    #[test]
    fn test_get_messages_for_gateway_initial_nonce(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal()), messages_count in any::<u64>().prop_map(|c| c % 100), max_number_of_returned_messages in any::<usize>().prop_map(|c| c % 1000)) {
        // Set up
        PARAMS.with(|p| {
            *p.borrow_mut() = WsInitParams {
                max_number_of_returned_messages,
                ..Default::default()
            }
        });
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(gateway_principal.clone())]));
        MESSAGES_FOR_GATEWAYS.with(|m| *m.borrow_mut() = {
            let mut m = HashMap::new();
            m.insert(gateway_principal.clone(), VecDeque::new());
            m
        });

        let test_client_key = test_utils::get_random_client_key();
        test_utils::add_messages_for_gateway(test_client_key, gateway_principal, messages_count);

        // Test
        let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, 0);
        let expected_start_index = if (messages_count as usize) > max_number_of_returned_messages {
            (messages_count as usize) - max_number_of_returned_messages
        } else {
            0
        };
        prop_assert_eq!(start_index, expected_start_index);
        prop_assert_eq!(end_index, messages_count as usize);

        // Clean up
        test_utils::clean_messages_for_gateway();
    }

    #[test]
    fn test_get_messages_for_gateway(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal()), messages_count in any::<u64>().prop_map(|c| c % 100)) {
        // Set up
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(gateway_principal.clone())]));
        MESSAGES_FOR_GATEWAYS.with(|m| *m.borrow_mut() = {
            let mut m = HashMap::new();
            m.insert(gateway_principal.clone(), VecDeque::new());
            m
        });

        let test_client_key = test_utils::get_random_client_key();
        test_utils::add_messages_for_gateway(test_client_key, gateway_principal, messages_count);

        // Test
        // add one to test the out of range index
        for i in 0..messages_count + 1 {
            let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
            let messages = get_messages_for_gateway(gateway_principal, start_index, end_index);

            // check if the messages returned are the ones we expect
            for (j, message) in messages.iter().enumerate() {
                let expected_key = get_message_for_gateway_key(gateway_principal.clone(), (start_index + j) as u64);
                prop_assert_eq!(&message.key, &expected_key);
            }
        }

        // Clean up
        test_utils::clean_messages_for_gateway();
    }

    #[test]
    fn test_check_is_registered_gateway(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
        // Set up
        REGISTERED_GATEWAYS.with(|p| *p.borrow_mut() = Some(vec![RegisteredGateway::new(test_gateway_principal.clone())]));

        let actual_result = check_is_registered_gateway(test_gateway_principal);
        prop_assert!(actual_result.is_ok());

        let other_principal = test_utils::generate_random_principal();
        let actual_result = check_is_registered_gateway(other_principal);
        prop_assert_eq!(actual_result.err(), Some(String::from("caller is not one of the authorized gateways that have been registered during CDK initialization")));
    }

    #[test]
    fn test_serialize_websocket_message(test_msg_bytes in any::<Vec<u8>>(), test_sequence_num in any::<u64>(), test_timestamp in any::<u64>()) {
        // TODO: add more tests, in which we check the serialized message
        let websocket_message = WebsocketMessage {
            client_key: test_utils::get_random_client_key(),
            sequence_num: test_sequence_num,
            timestamp: test_timestamp,
            is_service_message: false,
            content: test_msg_bytes,
        };

        let serialized_message = websocket_message.cbor_serialize();
        assert!(serialized_message.is_ok()); // not so useful as a test
    }
}
