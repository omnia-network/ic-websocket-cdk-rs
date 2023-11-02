/// The maximum number of messages returned by the **ws_get_messages** method.
pub const DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES: u64 = 20;

/// Add more messages than the max to check the indexes and limits.
///
/// Value: [DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES] + `2`
pub const SEND_MESSAGES_COUNT: u64 = DEFAULT_TEST_MAX_NUMBER_OF_RETURNED_MESSAGES + 2;

/// The interval between sending acks from the canister.
/// Set to a high value to make sure the canister doesn't reset the client while testing other functions.
///
/// Value: `300_000` = 5 minutes
pub const DEFAULT_TEST_SEND_ACK_INTERVAL_MS: u64 = 300_000;

/// The interval between keep alive checks in the canister.
/// Set to a high value to make sure the canister doesn't reset the client while testing other functions.
///
/// Value: `120_000` (2 minutes)
pub const DEFAULT_TEST_KEEP_ALIVE_TIMEOUT_MS: u64 = 120_000;
