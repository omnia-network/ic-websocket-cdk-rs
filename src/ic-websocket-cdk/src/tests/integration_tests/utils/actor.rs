use candid::{decode_one, encode_one, Principal};
use pocket_ic::WasmResult;

use super::{clients::GATEWAY_1, messages::AppMessage, test_env::get_test_env};

pub mod ws_open {
    use std::ops::Deref;

    use crate::{CanisterWsOpenArguments, CanisterWsOpenResult, ClientKey};

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_open(caller: &Principal, args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
        let canister_id = get_test_env().canister_id;
        let res = get_test_env()
            .pic
            .update_call(canister_id, *caller, "ws_open", encode_one(args).unwrap())
            .expect("Failed to call canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    /// Same as [call_ws_open].
    ///
    /// # Panics
    /// If [call_ws_open] panics or if the call returns an error variant.
    fn call_ws_open_with_panic(caller: &Principal, args: CanisterWsOpenArguments) {
        match call_ws_open(caller, args) {
            CanisterWsOpenResult::Ok(_) => {},
            CanisterWsOpenResult::Err(err) => panic!("failed ws_open: {:?}", err),
        }
    }

    /// See [call_ws_open_with_panic]. Uses [GATEWAY_1] as the gateway.
    pub(in crate::tests::integration_tests) fn call_ws_open_for_client_key_with_panic(
        client_key: &ClientKey,
    ) {
        let args = CanisterWsOpenArguments {
            client_nonce: client_key.client_nonce,
            gateway_principal: GATEWAY_1.deref().to_owned(),
        };
        call_ws_open_with_panic(&client_key.client_principal, args);
    }

    /// See [call_ws_open_with_panic]. Uses [GATEWAY_1] as the gateway.
    pub(in crate::tests::integration_tests) fn call_ws_open_for_client_key_and_gateway_with_panic(
        client_key: &ClientKey,
        gateway_principal: Principal,
    ) {
        let args = CanisterWsOpenArguments {
            client_nonce: client_key.client_nonce,
            gateway_principal,
        };
        call_ws_open_with_panic(&client_key.client_principal, args);
    }
}

pub mod ws_message {
    use crate::{CanisterWsMessageArguments, CanisterWsMessageResult};

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_message(
        caller: &Principal,
        args: CanisterWsMessageArguments,
    ) -> CanisterWsMessageResult {
        let canister_id = get_test_env().canister_id;
        let res = get_test_env()
            .pic
            .update_call(
                canister_id,
                *caller,
                "ws_message",
                encode_one(args).unwrap(),
            )
            .expect("Failed to call canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    /// Same as [call_ws_message].
    ///
    /// # Panics
    /// If [call_ws_message] panics or if the call returns an error variant.
    pub fn call_ws_message_with_panic(caller: &Principal, args: CanisterWsMessageArguments) {
        match call_ws_message(caller, args) {
            CanisterWsMessageResult::Ok(_) => {},
            CanisterWsMessageResult::Err(err) => panic!("failed ws_message: {:?}", err),
        }
    }
}

pub mod ws_close {
    use crate::{CanisterWsCloseArguments, CanisterWsCloseResult};

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_close(
        caller: &Principal,
        args: CanisterWsCloseArguments,
    ) -> CanisterWsCloseResult {
        let canister_id = get_test_env().canister_id;
        let res = get_test_env()
            .pic
            .update_call(canister_id, *caller, "ws_close", encode_one(args).unwrap())
            .expect("Failed to call canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    /// Same as [call_ws_close].
    ///
    /// # Panics
    /// If [call_ws_close] panics or if the call returns an error variant.
    pub fn call_ws_close_with_panic(caller: &Principal, args: CanisterWsCloseArguments) {
        match call_ws_close(caller, args) {
            CanisterWsCloseResult::Ok(_) => {},
            CanisterWsCloseResult::Err(err) => panic!("failed ws_close: {:?}", err),
        }
    }
}

pub mod ws_get_messages {
    use crate::{
        types::CanisterOutputCertifiedMessages, CanisterWsGetMessagesArguments,
        CanisterWsGetMessagesResult,
    };

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_get_messages(
        caller: &Principal,
        args: CanisterWsGetMessagesArguments,
    ) -> CanisterWsGetMessagesResult {
        let canister_id = get_test_env().canister_id;
        let res = get_test_env()
            .pic
            .query_call(
                canister_id,
                *caller,
                "ws_get_messages",
                encode_one(args).unwrap(),
            )
            .expect("Failed to call canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    /// Same as [call_ws_get_messages].
    ///
    /// # Panics
    /// If [call_ws_get_messages] panics or if the call returns an error variant.
    pub fn call_ws_get_messages_with_panic(
        caller: &Principal,
        args: CanisterWsGetMessagesArguments,
    ) -> CanisterOutputCertifiedMessages {
        match call_ws_get_messages(caller, args) {
            CanisterWsGetMessagesResult::Ok(res) => res,
            CanisterWsGetMessagesResult::Err(err) => panic!("failed ws_get_messages: {:?}", err),
        }
    }
}

pub mod ws_send {
    use crate::{CanisterWsSendResult, ClientPrincipal};
    use candid::encode_args;

    use super::*;

    /// (`ClientPrincipal`, `Vec<Vec<u8>>`)
    type WsSendArguments = (ClientPrincipal, Vec<Vec<u8>>);

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub(crate) fn call_ws_send(
        send_to_principal: &ClientPrincipal,
        messages: Vec<AppMessage>,
    ) -> CanisterWsSendResult {
        let messages: Vec<Vec<u8>> = messages.iter().map(|m| encode_one(m).unwrap()).collect();
        let args: WsSendArguments = (send_to_principal.clone(), messages);
        let canister_id = get_test_env().canister_id;
        let res = get_test_env()
            .pic
            .update_call(
                canister_id,
                Principal::anonymous(),
                "ws_send",
                encode_args(args).unwrap(),
            )
            .expect("Failed to call canister");
        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    /// Same as [call_ws_send].
    ///
    /// # Panics
    /// If [call_ws_send] panics or if the call returns an error variant.
    pub(crate) fn call_ws_send_with_panic(
        send_to_principal: &ClientPrincipal,
        messages: Vec<AppMessage>,
    ) {
        match call_ws_send(send_to_principal, messages) {
            CanisterWsSendResult::Ok(_) => {},
            CanisterWsSendResult::Err(err) => panic!("failed ws_send: {:?}", err),
        }
    }
}
