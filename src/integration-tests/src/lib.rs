#![cfg(test)]

use candid::Principal;
use lazy_static::lazy_static;
use pocket_ic::PocketIc;

use crate::wasm::load_canister_wasm_from_bin;

mod actor;
mod clients;
mod messages;
mod utils;
mod wasm;

mod ws_open;

lazy_static! {
    pub static ref TEST_ENV: TestEnv = TestEnv::new();
}

pub struct TestEnv {
    pub pic: PocketIc,
    pub canister_id: Principal,
}

type CanisterInitArgs<'a> = (&'a str, u64, u64, u64);

impl TestEnv {
    pub fn new() -> Self {
        let pic = PocketIc::new();
        let canister_id = pic.create_canister(None);
        pic.add_cycles(canister_id, 1_000_000_000_000_000);
        let wasm_bytes = load_canister_wasm_from_bin("test_canister.wasm");
        println!("wasm_bytes: {:?}", wasm_bytes.len());
        let arguments: CanisterInitArgs = (
            "i3gux-m3hwt-5mh2w-t7wwm-fwx5j-6z6ht-hxguo-t4rfw-qp24z-g5ivt-2qe",
            10,
            300_000,
            300_000,
        );
        pic.install_canister(
            canister_id,
            wasm_bytes,
            candid::encode_args(arguments).unwrap(),
            None,
        );
        Self { pic, canister_id }
    }
}
