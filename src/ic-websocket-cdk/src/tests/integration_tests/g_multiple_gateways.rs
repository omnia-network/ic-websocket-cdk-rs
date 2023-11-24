use proptest::prelude::*;

use crate::{
    tests::{
        common,
        integration_tests::utils::messages::get_service_message_content_from_canister_message,
    },
    CanisterOutputCertifiedMessages, CanisterWsCloseArguments, CanisterWsGetMessagesArguments,
    CanisterWsGetMessagesResult, GatewayPrincipal, WebsocketServiceMessageContent,
};

use super::utils::{
    actor::{
        ws_close::call_ws_close_with_panic,
        ws_get_messages::call_ws_get_messages,
        ws_open::call_ws_open_for_client_key_and_gateway_with_panic,
        ws_send::{call_ws_send_with_panic, AppMessage},
    },
    test_env::get_test_env,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_1_client_can_switch_to_another_gateway(
        ref client_key in any::<u8>().prop_map(|_| common::get_random_client_key()),
        gateways in any::<Vec<u8>>().prop_map(|_| (0..2).map(|_| common::generate_random_principal()).collect::<Vec<GatewayPrincipal>>()),
    ) {
        let first_gateway = &gateways[0];
        let second_gateway = &gateways[1];
        get_test_env().reset_canister_with_default_params();
        // open a connection for client
        call_ws_open_for_client_key_and_gateway_with_panic(&client_key, *first_gateway);
        // simulate canister sending messages to client
        call_ws_send_with_panic(
            &client_key.client_principal,
            (0..10)
                .map(|i| AppMessage {
                    text: format!("test{}", i),
                })
                .collect(),
        );

        // test
        // gateway 1 can poll the messages
        let res_gateway_1 = call_ws_get_messages(
            first_gateway,
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        match res_gateway_1 {
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
                prop_assert_eq!(messages.len() as u64, 10 + 1); // +1 for the open service message
            },
            _ => panic!("unexpected result"),
        };
        // gateway 2 has no messages
        let res_gateway_2 = call_ws_get_messages(
            second_gateway,
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        match res_gateway_2 {
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
                prop_assert_eq!(messages.len() as u64, 0);
            },
            _ => panic!("unexpected result"),
        };

        // client disconnects, so gateway 1 closes the connection
        call_ws_close_with_panic(
            first_gateway,
            CanisterWsCloseArguments {
                client_key: client_key.clone(),
            },
        );
        // client reopens connection with gateway 2
        call_ws_open_for_client_key_and_gateway_with_panic(&client_key, *second_gateway);
        // gateway 2 now has the open message
        let res_gateway_2 = call_ws_get_messages(
            second_gateway,
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        match res_gateway_2 {
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
                let first_message = &messages[0];
                prop_assert_eq!(&first_message.client_key, client_key);
                let open_message = get_service_message_content_from_canister_message(first_message);
                match open_message {
                    WebsocketServiceMessageContent::OpenMessage(open_message) => {
                        prop_assert_eq!(open_message.client_key, client_key.clone());
                    },
                    _ => panic!("Expected OpenMessage"),
                }
            },
            _ => panic!("unexpected result"),
        };

        // simulate canister sending other messages to client
        call_ws_send_with_panic(
            &client_key.client_principal,
            (0..10)
                .map(|i| AppMessage {
                    text: format!("test{}", i + 10),
                })
                .collect(),
        );
        let res_gateway_2 = call_ws_get_messages(
            second_gateway,
            CanisterWsGetMessagesArguments { nonce: 0 },
        );
        match res_gateway_2 {
            CanisterWsGetMessagesResult::Ok(CanisterOutputCertifiedMessages { messages, .. }) => {
                prop_assert_eq!(messages.len() as u64, 10 + 1); // +1 for the open service message
            },
            _ => panic!("unexpected result"),
        };
    }
}
