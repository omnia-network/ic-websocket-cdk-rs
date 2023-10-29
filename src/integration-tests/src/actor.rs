use candid::{decode_one, encode_one, Principal};
use ic_websocket_cdk::{
    CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult, CanisterWsOpenArguments,
    CanisterWsOpenResult, ClientKey,
};
use pocket_ic::WasmResult;

use crate::TEST_ENV;

pub mod ws_open {
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

pub mod ws_get_messages {
    use super::*;

    pub fn call_ws_get_messages(
        caller: Principal,
        args: CanisterWsGetMessagesArguments,
    ) -> CanisterWsGetMessagesResult {
        let res = TEST_ENV
            .pic
            .query_call(
                TEST_ENV.canister_id,
                caller,
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