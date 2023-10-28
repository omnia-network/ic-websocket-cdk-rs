use candid::{decode_one, encode_one, Principal};
use ic_websocket_cdk::{
    CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult, CanisterWsOpenArguments,
    CanisterWsOpenResult,
};
use pocket_ic::WasmResult;

use crate::TEST_ENV;

pub fn call_ws_open(caller: Principal, args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
    let res = TEST_ENV
        .pic
        .update_call(
            TEST_ENV.canister_id,
            caller,
            "ws_open",
            encode_one(args).unwrap(),
        )
        .expect("Failed to call counter canister");

    match res {
        WasmResult::Reply(bytes) => decode_one(&bytes).unwrap(),
        _ => panic!("Expected reply"),
    }
}

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
