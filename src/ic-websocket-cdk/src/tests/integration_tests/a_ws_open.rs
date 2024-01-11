use proptest::prelude::*;
use std::ops::Deref;

use crate::{
    errors::WsError, tests::integration_tests::utils::actor::wipe::call_wipe,
    CanisterOutputCertifiedMessages, CanisterOutputMessage, CanisterWsGetMessagesArguments,
    CanisterWsOpenArguments, CanisterWsOpenResult, ClientKey, WebsocketServiceMessageContent,
    DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES,
};
use candid::Principal;

use super::utils::{
    actor::{ws_get_messages::call_ws_get_messages_with_panic, ws_open::call_ws_open},
    clients::{generate_random_client_nonce, CLIENT_1_KEY, GATEWAY_1},
    messages::get_service_message_content_from_canister_message,
};

#[test]
fn test_1_fail_for_an_anonymous_client() {
    let args = CanisterWsOpenArguments {
        client_nonce: generate_random_client_nonce(),
        gateway_principal: GATEWAY_1.deref().to_owned(),
    };
    let res = call_ws_open(&Principal::anonymous(), args);
    assert_eq!(
        res,
        CanisterWsOpenResult::Err(WsError::AnonymousPrincipalNotAllowed.to_string()),
    );
}

#[test]
fn test_2_should_open_a_connection() {
    let client_1_key = CLIENT_1_KEY.deref();
    let args = CanisterWsOpenArguments {
        client_nonce: client_1_key.client_nonce,
        gateway_principal: GATEWAY_1.deref().to_owned(),
    };
    let res = call_ws_open(&client_1_key.client_principal, args);
    assert_eq!(res, CanisterWsOpenResult::Ok(()));

    let CanisterOutputCertifiedMessages { messages, .. } =
        call_ws_get_messages_with_panic(GATEWAY_1.deref(), CanisterWsGetMessagesArguments::new(0));

    let first_message = &messages[0];
    assert_eq!(first_message.client_key, *client_1_key);
    let open_message = get_service_message_content_from_canister_message(first_message);
    match open_message {
        WebsocketServiceMessageContent::OpenMessage(open_message) => {
            assert_eq!(open_message.client_key, *client_1_key);
        },
        _ => panic!("Expected OpenMessage"),
    }
}

#[test]
fn test_3_fails_for_a_client_with_the_same_nonce() {
    let client_1_key = CLIENT_1_KEY.deref();
    let args = CanisterWsOpenArguments {
        client_nonce: client_1_key.client_nonce,
        gateway_principal: GATEWAY_1.deref().to_owned(),
    };
    let res = call_ws_open(&client_1_key.client_principal, args);
    assert_eq!(
        res,
        CanisterWsOpenResult::Err(
            WsError::ClientKeyAlreadyConnected {
                client_key: client_1_key
            }
            .to_string()
        ),
    );

    // reset canister for the next test
    call_wipe(None);
}

proptest! {
    // avoid going over the max returned messages limit by using `DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES`
    #![proptest_config(ProptestConfig::with_cases(DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES as u32))]

    #[test]
    fn test_4_should_open_a_connection_for_the_same_client_with_a_different_nonce(test_client_nonce in any::<u64>().prop_map(|_| generate_random_client_nonce())) {
        let client_key = ClientKey::new(
            CLIENT_1_KEY.deref().client_principal,
            test_client_nonce
        );
        let args = CanisterWsOpenArguments {
            client_nonce: client_key.client_nonce,
            gateway_principal: GATEWAY_1.deref().to_owned(),
        };
        let res = call_ws_open(&client_key.client_principal, args);
        assert_eq!(res, CanisterWsOpenResult::Ok(()));

        let CanisterOutputCertifiedMessages { messages, .. } = call_ws_get_messages_with_panic(
            GATEWAY_1.deref(),
            CanisterWsGetMessagesArguments::new(0),
        );

        let service_message_for_client = messages
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
    }
}
