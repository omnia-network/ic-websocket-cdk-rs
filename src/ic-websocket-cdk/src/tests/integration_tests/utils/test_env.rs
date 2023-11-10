use std::time::{Duration, SystemTime};

use candid::Principal;
use lazy_static::lazy_static;
use pocket_ic::PocketIc;

use super::{
    clients::GATEWAY_1,
    constants::{
        DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS, DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
        DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
    },
    wasm::load_canister_wasm_from_bin,
};

lazy_static! {
    pub static ref TEST_ENV: TestEnv = TestEnv::new();
}

pub struct TestEnv {
    pub pic: PocketIc,
    pub canister_id: Principal,
    canister_init_args: CanisterInitArgs,
    wasm_module: Vec<u8>,
    root_ic_key: Vec<u8>,
}

type AuthorizedGateways = Vec<String>;
/// (`gateway_principal`, `max_number_or_returned_messages`, `send_ack_interval_ms`, `send_ack_timeout_ms`)
type CanisterInitArgs = (AuthorizedGateways, u64, u64, u64);

impl TestEnv {
    pub fn new() -> Self {
        let pic = PocketIc::new();

        // set ic time to current time
        pic.set_time(SystemTime::now());

        let canister_id = pic.create_canister(None);
        pic.add_cycles(canister_id, 1_000_000_000_000_000);

        let wasm_bytes = load_canister_wasm_from_bin("test_canister.wasm");

        let authorized_gateways = vec![GATEWAY_1.to_string()];
        let arguments: CanisterInitArgs = (
            authorized_gateways,
            DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
            DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
            DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
        );
        pic.install_canister(
            canister_id,
            wasm_bytes.clone(),
            candid::encode_args(arguments.clone()).unwrap(),
            None,
        );

        let root_ic_key = pic.root_key();

        Self {
            pic,
            canister_id,
            canister_init_args: arguments,
            wasm_module: wasm_bytes,
            root_ic_key,
        }
    }

    pub fn reset_canister(
        &self,
        max_number_or_returned_messages: u64,
        send_ack_interval_ms: u64,
        keep_alive_delay_ms: u64,
    ) {
        let arguments: CanisterInitArgs = (
            self.canister_init_args.0.clone(),
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

    pub fn reset_canister_with_default_params(&self) {
        self.reset_canister(
            DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
            DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
            DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS,
        );
    }

    /// Returns the current time of the canister in nanoseconds.
    pub fn get_canister_time(&self) -> u64 {
        self.pic
            .get_time()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    pub fn get_root_ic_key(&self) -> Vec<u8> {
        self.root_ic_key.clone()
    }

    pub fn advance_canister_time_ms(&self, ms: u64) {
        self.pic.advance_time(Duration::from_millis(ms));
        // produce and advance by one block to fire eventual timers
        self.pic.tick();
    }
}
