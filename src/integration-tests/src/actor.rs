use candid::{decode_one, encode_one, Principal};
use pocket_ic::WasmResult;

use crate::TEST_ENV;

pub mod ws_open {
    use ic_websocket_cdk::{CanisterWsOpenArguments, CanisterWsOpenResult, ClientKey};

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_open(caller: &Principal, args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
        let res = TEST_ENV
            .pic
            .update_call(
                TEST_ENV.canister_id,
                *caller,
                "ws_open",
                encode_one(args).unwrap(),
            )
            .expect("Failed to call canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    /// Same as [call_ws_open] but panics if the call returns an error variant.
    fn call_ws_open_with_panic(caller: &Principal, args: CanisterWsOpenArguments) {
        match call_ws_open(caller, args) {
            CanisterWsOpenResult::Ok(_) => {},
            CanisterWsOpenResult::Err(err) => panic!("failed ws_open: {:?}", err),
        }
    }

    /// See [call_ws_open_with_panic].
    pub fn call_ws_open_for_client_key_with_panic(client_key: &ClientKey) {
        let args = CanisterWsOpenArguments {
            client_nonce: client_key.client_nonce,
        };
        call_ws_open_with_panic(&client_key.client_principal, args);
    }
}

pub mod ws_message {
    use ic_websocket_cdk::{CanisterWsMessageArguments, CanisterWsMessageResult};

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_message(
        caller: &Principal,
        args: CanisterWsMessageArguments,
    ) -> CanisterWsMessageResult {
        let res = TEST_ENV
            .pic
            .update_call(
                TEST_ENV.canister_id,
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
}

pub mod ws_close {
    use ic_websocket_cdk::{CanisterWsCloseArguments, CanisterWsCloseResult};

    use super::*;

    /// # Panics
    /// if the call returns a [WasmResult::Reject].
    pub fn call_ws_close(
        caller: &Principal,
        args: CanisterWsCloseArguments,
    ) -> CanisterWsCloseResult {
        let res = TEST_ENV
            .pic
            .update_call(
                TEST_ENV.canister_id,
                *caller,
                "ws_close",
                encode_one(args).unwrap(),
            )
            .expect("Failed to call canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }
}

pub mod ws_get_messages {
    use ic_websocket_cdk::{CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult};

    use super::*;

    pub fn call_ws_get_messages(
        caller: &Principal,
        args: CanisterWsGetMessagesArguments,
    ) -> CanisterWsGetMessagesResult {
        let res = TEST_ENV
            .pic
            .query_call(
                TEST_ENV.canister_id,
                *caller,
                "ws_get_messages",
                encode_one(args).unwrap(),
            )
            .expect("Failed to call counter canister");

        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }
}

pub mod ws_send {
    use candid::{encode_args, CandidType};
    use ic_websocket_cdk::CanisterWsSendResult;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(CandidType, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct AppMessage {
        pub text: String,
    }

    /// (`Principal`, `Vec<Vec<u8>>`)
    type WsSendArguments = (Principal, Vec<Vec<u8>>);

    pub fn call_ws_send(
        send_to_principal: &Principal,
        messages: Vec<AppMessage>,
    ) -> CanisterWsSendResult {
        let messages: Vec<Vec<u8>> = messages.iter().map(|m| encode_one(m).unwrap()).collect();
        let args: WsSendArguments = (send_to_principal.clone(), messages);
        let res = TEST_ENV
            .pic
            .update_call(
                TEST_ENV.canister_id,
                Principal::anonymous(),
                "ws_send",
                encode_args(args).unwrap(),
            )
            .expect("Failed to call counter canister");
        match res {
            WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
            _ => panic!("Expected reply"),
        }
    }

    pub fn call_ws_send_with_panic(send_to_principal: &Principal, messages: Vec<AppMessage>) {
        let res = call_ws_send(send_to_principal, messages);
        match res {
            CanisterWsSendResult::Ok(_) => {},
            CanisterWsSendResult::Err(err) => panic!("failed ws_send: {:?}", err),
        }
    }
}
