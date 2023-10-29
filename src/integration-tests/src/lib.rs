#![cfg(test)]

use candid::Principal;
use lazy_static::lazy_static;
use pocket_ic::PocketIc;

use crate::wasm::load_canister_wasm_from_bin;

mod actor;
mod clients;
mod constants;
mod messages;
mod utils;
mod wasm;

mod tests;

lazy_static! {
    pub static ref TEST_ENV: TestEnv<'static> = TestEnv::new();
}

pub struct TestEnv<'a> {
    pub pic: PocketIc,
    pub canister_id: Principal,
    canister_init_args: CanisterInitArgs<'a>,
    wasm_module: Vec<u8>,
}

/// (`gateway_principal`, `max_number_or_returned_messages`, `send_ack_interval_ms`, `send_ack_timeout_ms`)
type CanisterInitArgs<'a> = (&'a str, u64, u64, u64);

impl TestEnv<'_> {
    pub fn new() -> Self {
        let pic = PocketIc::new();
        let canister_id = pic.create_canister(None);
        pic.add_cycles(canister_id, 1_000_000_000_000_000);

        let wasm_bytes = load_canister_wasm_from_bin("test_canister.wasm");
        let arguments: CanisterInitArgs = (
            "i3gux-m3hwt-5mh2w-t7wwm-fwx5j-6z6ht-hxguo-t4rfw-qp24z-g5ivt-2qe",
            10,
            300_000,
            300_000,
        );
        pic.install_canister(
            canister_id,
            wasm_bytes.clone(),
            candid::encode_args(arguments.clone()).unwrap(),
            None,
        );

        Self {
            pic,
            canister_id,
            canister_init_args: arguments,
            wasm_module: wasm_bytes,
        }
    }

    pub fn reset_canister(
        &self,
        max_number_or_returned_messages: u64,
        send_ack_interval_ms: u64,
        keep_alive_delay_ms: u64,
    ) {
        let arguments: CanisterInitArgs = (
            self.canister_init_args.0,
            max_number_or_returned_messages,
            send_ack_interval_ms,
            keep_alive_delay_ms,
        );
        let res = self.pic.reinstall_canister(
            self.canister_id,
            self.wasm_module.to_owned(),
            candid::encode_args(arguments).unwrap(),
            None,
        );

        match res {
            Ok(_) => {},
            Err(err) => {
                panic!("Failed to reset canister: {:?}", err);
            },
        }
    }
}
