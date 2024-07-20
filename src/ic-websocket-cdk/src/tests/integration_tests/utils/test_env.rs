use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::{Duration, SystemTime},
};

use candid::Principal;
use lazy_static::lazy_static;
use pocket_ic::{PocketIc, PocketIcBuilder};

use super::wasm::{load_canister_wasm_from_bin, load_canister_wasm_from_path};

/// The maximum number of messages returned by the **ws_get_messages** method.
pub const DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES: u64 = 50;

/// The interval between sending acks from the canister.
/// Set to a high value to make sure the canister doesn't reset the client while testing other functions.
///
/// Value: `300_000` = 5 minutes
pub const DEFAULT_TEST_SEND_ACK_INTERVAL_MS: u64 = 300_000;

/// (`max_number_or_returned_messages`, `send_ack_interval_ms`)
pub type CanisterInitArgs = (u64, u64);

pub const DEFAULT_CANISTER_INIT_ARGS: CanisterInitArgs = (
    DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES,
    DEFAULT_TEST_SEND_ACK_INTERVAL_MS,
);

lazy_static! {
    pub static ref TEST_ENV: Mutex<TestEnv> = Mutex::new(TestEnv::new());
    static ref TEST_CANISTER_WASM_MODULE: Vec<u8> = match std::env::var("TEST_CANISTER_WASM_PATH") {
        Ok(path) => load_canister_wasm_from_path(&PathBuf::from(path)),
        Err(_) => load_canister_wasm_from_bin("test_canister.wasm"),
    };
}

pub fn get_test_env<'a>() -> MutexGuard<'a, TestEnv> {
    TEST_ENV.lock().unwrap()
}

pub struct TestEnv {
    pub pic: PocketIc,
    test_canister_id: Principal,
    root_ic_key: Vec<u8>,
}

impl TestEnv {
    pub fn new() -> Self {
        let pic = PocketIcBuilder::new()
            // NNS subnet needed to retrieve the root key
            .with_nns_subnet()
            .with_application_subnet()
            .build();

        // set ic time to current time
        pic.set_time(SystemTime::now());

        let app_subnet = pic.topology().get_app_subnets()[0];
        let canister_id = pic.create_canister_on_subnet(None, None, app_subnet);
        pic.add_cycles(canister_id, 1_000_000_000_000_000);

        pic.install_canister(
            canister_id,
            TEST_CANISTER_WASM_MODULE.clone(),
            candid::encode_args(DEFAULT_CANISTER_INIT_ARGS).unwrap(),
            None,
        );

        let root_ic_key = pic.root_key().unwrap();

        Self {
            pic,
            test_canister_id: canister_id,
            root_ic_key,
        }
    }

    pub fn get_test_canister_id(&self) -> Principal {
        self.test_canister_id
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
        // produce and advance by some blocks to fire eventual timers
        // see https://forum.dfinity.org/t/pocketic-multi-subnet-canister-testing/24901/4
        for _ in 0..10 {
            self.pic.tick();
        }
    }
}
