use candid::{CandidType, Principal};
use ed25519_compact::{PublicKey, Signature};
#[cfg(not(test))]
use ic_cdk::api::time;
use ic_cdk::api::{caller, data_certificate, set_certified_data};
use ic_cdk_timers::set_timer;
use ic_certified_map::{labeled, labeled_hash, AsHashTree, Hash as ICHash, RbTree};
use serde::{Deserialize, Serialize};
use serde_cbor::{from_slice, Serializer};
use sha2::{Digest, Sha256};
use std::time::Duration;
use std::{
    cell::RefCell,
    collections::VecDeque,
    collections::{HashMap, HashSet},
    convert::AsRef,
};

mod logger;

/// The label used when constructing the certification tree.
const LABEL_WEBSOCKET: &[u8] = b"websocket";
/// The maximum number of messages returned by [ws_get_messages] at each poll.
const MAX_NUMBER_OF_RETURNED_MESSAGES: usize = 10;
/// The delay between two consecutive checks if the registered gateway is still alive.
const CHECK_REGISTERED_GATEWAY_DELAY_NS: u64 = 60_000_000_000; // 60 seconds
/// (**Used for integration tests**) The delay between two consecutive checks if the registered gateway is still alive.
const CHECK_REGISTERED_GATEWAY_DELAY_NS_TEST: u64 = 15_000_000_000; // 15 seconds

pub type ClientPublicKey = Vec<u8>;

/// The result of [ws_register].
pub type CanisterWsRegisterResult = Result<(), String>;
/// The result of [ws_open].
pub type CanisterWsOpenResult = Result<CanisterWsOpenResultValue, String>;
/// The result of [ws_message].
pub type CanisterWsMessageResult = Result<(), String>;
/// The result of [ws_get_messages].
pub type CanisterWsGetMessagesResult = Result<CanisterOutputCertifiedMessages, String>;
/// The result of [ws_send].
pub type CanisterWsSendResult = Result<(), String>;
/// The result of [ws_close].
pub type CanisterWsCloseResult = Result<(), String>;

/// The Ok value of CanisterWsOpenResult returned by [ws_open].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsOpenResultValue {
    client_key: ClientPublicKey,
    canister_id: Principal,
    nonce: u64,
}

/// The arguments for [ws_register].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsRegisterArguments {
    #[serde(with = "serde_bytes")]
    client_key: ClientPublicKey,
}

/// The arguments for [ws_open].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsOpenArguments {
    #[serde(with = "serde_bytes")]
    content: Vec<u8>,
    #[serde(with = "serde_bytes")]
    sig: Vec<u8>,
}

/// The arguments for [ws_close].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsCloseArguments {
    #[serde(with = "serde_bytes")]
    client_key: ClientPublicKey,
}

/// The arguments for [ws_message].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsMessageArguments {
    msg: CanisterIncomingMessage,
}

/// The arguments for [ws_get_messages].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsGetMessagesArguments {
    nonce: u64,
}

/// The first message received by the canister in [ws_open].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
struct CanisterOpenMessageContent {
    #[serde(with = "serde_bytes")]
    client_key: ClientPublicKey,
    canister_id: Principal,
}

/// Message + signature from client, **relayed** by the WS Gateway.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct RelayedClientMessage {
    #[serde(with = "serde_bytes")]
    content: Vec<u8>,
    #[serde(with = "serde_bytes")]
    sig: Vec<u8>,
}

/// Message coming **directly** from client, not relayed by the WS Gateway.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct DirectClientMessage {
    message: Vec<u8>,
    client_key: ClientPublicKey,
}

/// Heartbeat message sent from the WS Gateway to the canister, so that the canister can
/// verify that the WS Gateway is still alive.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct GatewayStatusMessage {
    status_index: u64,
}

/// The variants of the possible messages received by the canister in [ws_message].
/// - **IcWebSocketEstablished**: message sent from WS Gateway to the canister to notify it about the
///                               establishment of the IcWebSocketConnection
/// - **IcWebSocketGatewayStatus**:      message sent from WS Gateway to the canister to notify it about the
///                               status of the IcWebSocketConnection
/// - **RelayedByGateway**:       message sent from the client to the WS Gateway (via WebSocket) and
///                               relayed to the canister by the WS Gateway
/// - **DirectlyFromClient**:     message sent from directly client so that it is not necessary to
///                               verify the signature
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
enum CanisterIncomingMessage {
    DirectlyFromClient(DirectClientMessage),
    RelayedByGateway(RelayedClientMessage),
    IcWebSocketEstablished(ClientPublicKey),
    IcWebSocketGatewayStatus(GatewayStatusMessage),
}

/// Messages exchanged through the WebSocket.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
struct WebsocketMessage {
    #[serde(with = "serde_bytes")]
    client_key: ClientPublicKey, // The client that the gateway will forward the message to or that sent the message.
    sequence_num: u64, // Both ways, messages should arrive with sequence numbers 0, 1, 2...
    timestamp: u64,    // Timestamp of when the message was made for the recipient to inspect.
    #[serde(with = "serde_bytes")]
    message: Vec<u8>, // Application message encoded in binary.
}

impl WebsocketMessage {
    /// Serializes the message into a Vec<u8>, using CBOR.
    fn cbor_serialize(&self) -> Result<Vec<u8>, String> {
        let mut data = vec![];
        let mut serializer = Serializer::new(&mut data);
        serializer.self_describe().map_err(|e| e.to_string())?;
        self.serialize(&mut serializer).map_err(|e| e.to_string())?;
        Ok(data)
    }
}

/// Element of the list of messages returned to the WS Gateway after polling.
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanisterOutputMessage {
    #[serde(with = "serde_bytes")]
    client_key: Vec<u8>, // The client that the gateway will forward the message to or that sent the message.
    #[serde(with = "serde_bytes")]
    content: Vec<u8>, // The message to be relayed, that contains the application message.
    key: String, // Key for certificate verification.
}

/// List of messages returned to the WS Gateway after polling.
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanisterOutputCertifiedMessages {
    messages: Vec<CanisterOutputMessage>, // List of messages.
    #[serde(with = "serde_bytes")]
    cert: Vec<u8>, // cert+tree constitute the certificate for all returned messages.
    #[serde(with = "serde_bytes")]
    tree: Vec<u8>, // cert+tree constitute the certificate for all returned messages.
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Contains data about the registered WS Gateway.
struct RegisteredGateway {
    /// The principal of the gateway.
    gateway_principal: Principal,
    /// The last time the gateway sent a heartbeat message.
    last_heartbeat: Option<u64>,
    /// The last status index received from the gateway.
    last_status_index: u64,
}

impl RegisteredGateway {
    /// Creates a new instance of RegisteredGateway.
    fn new(gateway_principal: Principal) -> Self {
        Self {
            gateway_principal,
            last_heartbeat: None,
            last_status_index: 0,
        }
    }

    /// Updates the registered gateway's status index with the given one.
    /// Sets the last heartbeat to the current time.
    fn update_status_index(&mut self, status_index: u64) -> Result<(), String> {
        if status_index <= self.last_status_index {
            if status_index == 0 {
                custom_print!("Gateway status index set to 0");
            } else {
                return Err("Gateway status index is equal to or behind the current one".to_owned());
            }
        }
        self.last_status_index = status_index;
        self.last_heartbeat = Some(get_current_time());
        Ok(())
    }

    /// Resets the registered gateway to the initial state.
    fn reset(&mut self) {
        self.last_heartbeat = None;
        self.last_status_index = 0;

        custom_print!("Gateway has been reset");
    }
}

fn get_current_time() -> u64 {
    #[cfg(test)]
    {
        0u64
    }
    #[cfg(not(test))]
    {
        time()
    }
}

/// Returns true if the canister is running in an integration test.
///
/// To run the canister in an integration test,
/// the `IC_WS_CDK_INTEGRATION_TEST` environment variable must be set to `1` at build time.
fn is_integration_test() -> bool {
    let integration_test = option_env!("IC_WS_CDK_INTEGRATION_TEST");
    integration_test.is_some() && integration_test.unwrap() == "1"
}

/// Returns the delay in nanoseconds between two consecutive checks if the registered gateway is still alive.
fn get_check_registered_gateway_delay_ns() -> u64 {
    if is_integration_test() {
        CHECK_REGISTERED_GATEWAY_DELAY_NS_TEST
    } else {
        CHECK_REGISTERED_GATEWAY_DELAY_NS
    }
}

/// The temporary clients that don't still have a connection open, based on the gateway status index at which they were registered.
/// The last two status indexes are kept in order to be able to handle the case when the gateway crashes and restarts.
struct TmpClients {
    second_last_index_clients: HashSet<ClientPublicKey>,
    last_index_clients: HashSet<ClientPublicKey>,
}

impl TmpClients {
    fn new() -> Self {
        Self {
            second_last_index_clients: HashSet::new(),
            last_index_clients: HashSet::new(),
        }
    }

    fn shift(&mut self) {
        self.second_last_index_clients = self.last_index_clients.clone();
        self.last_index_clients = HashSet::new();
    }

    fn clear(&mut self) {
        self.second_last_index_clients.clear();
        self.last_index_clients.clear();
    }

    fn insert(&mut self, client_key: ClientPublicKey) {
        self.last_index_clients.insert(client_key);
    }

    fn contain_client(&self, client_key: &ClientPublicKey) -> bool {
        self.last_index_clients.contains(client_key)
            || self.second_last_index_clients.contains(client_key)
    }

    fn remove(&mut self, client_key: &ClientPublicKey) {
        let is_removed = self.last_index_clients.remove(client_key);
        if !is_removed {
            self.second_last_index_clients.remove(client_key);
        }
    }
}

thread_local! {
    /// Maps the client's public key to the client's identity (anonymous if not authenticated).
    /* flexible */ static CLIENT_CALLER_MAP: RefCell<HashMap<ClientPublicKey, Principal>> = RefCell::new(HashMap::new());
    /// Maps the clients that still don't have a connection open, based on the gateway status index at which they were registered.
    /* flexible */ static TMP_CLIENTS: RefCell<TmpClients> = RefCell::new(TmpClients::new());
    /// Maps the client's public key to the sequence number to use for the next outgoing message (to that client).
    /* flexible */ static OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP: RefCell<HashMap<ClientPublicKey, u64>> = RefCell::new(HashMap::new());
    /// Maps the client's public key to the expected sequence number of the next incoming message (from that client).
    /* flexible */ static INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP: RefCell<HashMap<ClientPublicKey, u64>> = RefCell::new(HashMap::new());
    /// Keeps track of the Merkle tree used for certified queries
    /* flexible */ static CERT_TREE: RefCell<RbTree<String, ICHash>> = RefCell::new(RbTree::new());
    /// Keeps track of the principal of the WS Gateway which polls the canister
    /* flexible */ static REGISTERED_GATEWAY: RefCell<Option<RegisteredGateway>> = RefCell::new(None);
    /// Keeps track of the messages that have to be sent to the WS Gateway
    /* flexible */ static MESSAGES_FOR_GATEWAY: RefCell<VecDeque<CanisterOutputMessage>> = RefCell::new(VecDeque::new());
    /// Keeps track of the nonce which:
    /// - the WS Gateway uses to specify the first index of the certified messages to be returned when polling
    /// - the client uses as part of the path in the Merkle tree in order to verify the certificate of the messages relayed by the WS Gateway
    /* flexible */ static OUTGOING_MESSAGE_NONCE: RefCell<u64> = RefCell::new(0u64);
    /// The callback handlers for the WebSocket.
    /* flexible */ static HANDLERS: RefCell<WsHandlers> = RefCell::new(WsHandlers {
        on_open: None,
        on_message: None,
        on_close: None,
    });
}

/// Resets all RefCells to their initial state.
/// If there is a registered gateway, resets its state as well.
fn reset_internal_state() {
    // get the handlers to call the on_close handler for each client
    let handlers = HANDLERS.with(|state| state.borrow().clone());

    CLIENT_CALLER_MAP.with(|state| {
        let mut map = state.borrow_mut();
        // for each client, call the on_close handler before clearing the map
        for (client_key, _) in map.clone().iter() {
            // If a client registers while the gateway crashes and restarts, we have to keep the client in the map,
            // so that the ws_open invoked by the gateway doesn't fail.
            // To be sure that we retain the latest unregistered clients,
            // we keep all the clients that have registered after the last two times the gateway updated the status index
            if !is_client_in_tmp_clients(client_key) {
                handlers.call_on_close(OnCloseCallbackArgs {
                    client_key: client_key.clone(),
                });

                map.remove(client_key);
            }
        }
    });

    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().clear();
    });
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().clear();
    });
    CERT_TREE.with(|t| {
        t.replace(RbTree::new());
    });
    MESSAGES_FOR_GATEWAY.with(|m| *m.borrow_mut() = VecDeque::new());
    OUTGOING_MESSAGE_NONCE.with(|next_id| next_id.replace(0u64));
}

/// Resets the internal state of the IC WebSocket CDK.
///
/// **Note:** You should only call this function in tests.
pub fn wipe() {
    reset_internal_state();

    // if there is a registered gateway, reset its state
    REGISTERED_GATEWAY.with(|state| {
        let mut registered_gateway = state.borrow_mut();
        if let Some(v) = registered_gateway.as_mut() {
            v.reset();
        }
    });

    // remove all clients from the map
    CLIENT_CALLER_MAP.with(|map| {
        map.borrow_mut().clear();
    });

    // clear the temporary clients
    TMP_CLIENTS.with(|tmp_clients| {
        tmp_clients.borrow_mut().clear();
    });

    custom_print!("Internal state has been wiped!");
}

fn get_outgoing_message_nonce() -> u64 {
    OUTGOING_MESSAGE_NONCE.with(|n| n.borrow().clone())
}

fn increment_outgoing_message_nonce() {
    OUTGOING_MESSAGE_NONCE.with(|n| n.replace_with(|&mut old| old + 1));
}

fn put_client_caller(client_key: ClientPublicKey, caller: Principal) {
    CLIENT_CALLER_MAP.with(|map| {
        map.borrow_mut().insert(client_key.clone(), caller);
    });

    // add the client to the temporary clients
    insert_in_tmp_clients(client_key);
}

fn get_client_caller(client_key: &ClientPublicKey) -> Option<Principal> {
    CLIENT_CALLER_MAP.with(|map| Some(map.borrow().get(client_key)?.to_owned()))
}

fn initialize_registered_gateway(gateway_principal: &str) {
    REGISTERED_GATEWAY.with(|p| {
        let gateway_principal =
            Principal::from_text(gateway_principal).expect("invalid gateway principal");
        *p.borrow_mut() = Some(RegisteredGateway::new(gateway_principal));
    });
}

fn get_registered_gateway_principal() -> Principal {
    REGISTERED_GATEWAY.with(|g| {
        g.borrow()
            .expect("gateway should be initialized")
            .gateway_principal
    })
}

fn insert_in_tmp_clients(client_key: ClientPublicKey) {
    TMP_CLIENTS.with(|tmp_clients| {
        let mut tmp_clients = tmp_clients.borrow_mut();
        tmp_clients.insert(client_key);
    });
}

fn shift_tmp_clients() {
    TMP_CLIENTS.with(|tmp_clients| {
        let mut tmp_clients = tmp_clients.borrow_mut();
        tmp_clients.shift();
    });
}

fn is_client_in_tmp_clients(client_key: &ClientPublicKey) -> bool {
    TMP_CLIENTS.with(|tmp_clients| {
        let tmp_clients = tmp_clients.borrow();
        tmp_clients.contain_client(client_key)
    })
}

fn remove_client_from_tmp_clients(client_key: &ClientPublicKey) {
    TMP_CLIENTS.with(|tmp_clients| {
        let mut tmp_clients = tmp_clients.borrow_mut();
        tmp_clients.remove(client_key);
    });
}

/// Updates the registered gateway with the new status index.
/// If the status index is not greater than the current one, the function returns an error.
fn update_registered_gateway_status_index(status_index: u64) -> Result<(), String> {
    REGISTERED_GATEWAY.with(|state| {
        let mut registered_gateway = state.borrow_mut();

        if let Some(v) = registered_gateway.as_mut() {
            // if the current status index is > 0 and the new status index is 0, it means that the gateway has been restarted
            // in this case, we reset the internal state because all clients are not connected to the gateway anymore
            if v.last_status_index > 0 && status_index == 0 {
                reset_internal_state();

                v.reset();

                Ok(())
            } else {
                // update the temporary clients, shifting the last index clients to the second last index clients
                shift_tmp_clients();

                v.update_status_index(status_index)
            }
        } else {
            Err("no gateway registered".to_owned())
        }
    })
}

fn check_registered_client_key(client_key: &ClientPublicKey) -> Result<(), String> {
    if !CLIENT_CALLER_MAP.with(|map| map.borrow().contains_key(client_key)) {
        return Err(String::from(
            "client's public key has not been previously registered by client",
        ));
    }

    Ok(())
}

fn init_outgoing_message_to_client_num(client_key: ClientPublicKey) {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().insert(client_key, 0);
    });
}

fn get_outgoing_message_to_client_num(client_key: &ClientPublicKey) -> Result<u64, String> {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        let map = map.borrow();
        let num = *map.get(client_key).ok_or(String::from(
            "outgoing message to client num not initialized for client",
        ))?;
        Ok(num)
    })
}

fn increment_outgoing_message_to_client_num(client_key: &ClientPublicKey) -> Result<(), String> {
    let num = get_outgoing_message_to_client_num(client_key)?;
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        let mut map = map.borrow_mut();
        map.insert(client_key.clone(), num + 1);
        Ok(())
    })
}

fn init_expected_incoming_message_from_client_num(client_key: ClientPublicKey) {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().insert(client_key, 0);
    });
}

fn get_expected_incoming_message_from_client_num(
    client_key: &ClientPublicKey,
) -> Result<u64, String> {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        let num = *map.borrow().get(client_key).ok_or(String::from(
            "expected incoming message num not initialized for client",
        ))?;
        Ok(num)
    })
}

fn increment_expected_incoming_message_from_client_num(
    client_key: &ClientPublicKey,
) -> Result<(), String> {
    let num = get_expected_incoming_message_from_client_num(client_key)?;
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        let mut map = map.borrow_mut();
        map.insert(client_key.clone(), num + 1);
        Ok(())
    })
}

fn add_client(client_key: ClientPublicKey) {
    // initialize incoming client's message sequence number to 0
    init_expected_incoming_message_from_client_num(client_key.clone());
    // initialize outgoing message sequence number to 0
    init_outgoing_message_to_client_num(client_key.clone());

    // now that the client is registered, remove it from the temporary clients
    remove_client_from_tmp_clients(&client_key);
}

fn remove_client(client_key: ClientPublicKey) {
    CLIENT_CALLER_MAP.with(|map| {
        map.borrow_mut().remove(&client_key);
    });
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().remove(&client_key);
    });
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().remove(&client_key);
    });
}

fn get_message_for_gateway_key(gateway_principal: Principal, nonce: u64) -> String {
    gateway_principal.to_string() + "_" + &format!("{:0>20}", nonce.to_string())
}

fn get_messages_for_gateway_range(gateway_principal: Principal, nonce: u64) -> (usize, usize) {
    MESSAGES_FOR_GATEWAY.with(|m| {
        // smallest key used to determine the first message from the queue which has to be returned to the WS Gateway
        let smallest_key = get_message_for_gateway_key(gateway_principal, nonce);
        // partition the queue at the message which has the key with the nonce specified as argument to get_cert_messages
        let start_index = m.borrow().partition_point(|x| x.key < smallest_key);
        // message at index corresponding to end index is excluded
        let mut end_index = m.borrow().len();
        if end_index - start_index > MAX_NUMBER_OF_RETURNED_MESSAGES {
            end_index = start_index + MAX_NUMBER_OF_RETURNED_MESSAGES;
        }
        (start_index, end_index)
    })
}

fn get_messages_for_gateway(start_index: usize, end_index: usize) -> Vec<CanisterOutputMessage> {
    MESSAGES_FOR_GATEWAY.with(|m| {
        let mut messages: Vec<CanisterOutputMessage> = Vec::with_capacity(end_index - start_index);
        for index in start_index..end_index {
            messages.push(m.borrow().get(index).unwrap().clone());
        }
        messages
    })
}

/// Gets the messages in GATEWAY_MESSAGES starting from the one with the specified nonce
fn get_cert_messages(gateway_principal: Principal, nonce: u64) -> CanisterWsGetMessagesResult {
    let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, nonce);
    let messages = get_messages_for_gateway(start_index, end_index);

    if messages.is_empty() {
        return Ok(CanisterOutputCertifiedMessages {
            messages,
            cert: Vec::new(),
            tree: Vec::new(),
        });
    }

    let first_key = messages.first().unwrap().key.clone();
    let last_key = messages.last().unwrap().key.clone();
    let (cert, tree) = get_cert_for_range(&first_key, &last_key);

    Ok(CanisterOutputCertifiedMessages {
        messages,
        cert,
        tree,
    })
}

/// Checks if the caller of the method is the same as the one that was registered during the initialization of the CDK
fn check_is_registered_gateway(input_principal: Principal) -> Result<(), String> {
    let gateway_principal = get_registered_gateway_principal();
    // check if the caller is the same as the one that was registered during the initialization of the CDK
    if gateway_principal != input_principal {
        return Err(String::from(
            "caller is not the gateway that has been registered during CDK initialization",
        ));
    }
    Ok(())
}

fn put_cert_for_message(key: String, value: &Vec<u8>) {
    let root_hash = CERT_TREE.with(|tree| {
        let mut tree = tree.borrow_mut();
        tree.insert(key.clone(), Sha256::digest(value).into());
        labeled_hash(LABEL_WEBSOCKET, &tree.root_hash())
    });

    set_certified_data(&root_hash);
}

fn get_cert_for_range(first: &String, last: &String) -> (Vec<u8>, Vec<u8>) {
    CERT_TREE.with(|tree| {
        let tree = tree.borrow();
        let witness = tree.value_range(first.as_ref(), last.as_ref());
        let tree = labeled(LABEL_WEBSOCKET, witness);

        let mut data = vec![];
        let mut serializer = Serializer::new(&mut data);
        serializer.self_describe().unwrap();
        tree.serialize(&mut serializer).unwrap();
        (data_certificate().unwrap(), data)
    })
}

/// Schedules a timer to check if the registered gateway has sent a heartbeat recently.
///
/// The timer delay is given by the [get_check_registered_gateway_delay_ms] function.
///
/// The timer callback is [check_registered_gateway_timer_callback].
fn schedule_registered_gateway_check() {
    set_timer(
        Duration::from_nanos(get_check_registered_gateway_delay_ns()),
        check_registered_gateway_timer_callback,
    );
}

/// Checks if the registered gateway has sent a heartbeat recently.
/// If not, this means that the gateway has been restarted and all clients registered have been disconnected.
/// In this case, all internal IC WebSocket CDK state is reset.
///
/// At the end, a new timer is scheduled to check again if the registered gateway has sent a heartbeat recently.
fn check_registered_gateway_timer_callback() {
    REGISTERED_GATEWAY.with(|state| {
        let mut registered_gateway = state.borrow_mut();
        if let Some(v) = registered_gateway.as_mut() {
            if let Some(last_heartbeat) = v.last_heartbeat {
                if get_current_time() - last_heartbeat > get_check_registered_gateway_delay_ns() {
                    custom_print!("[timer-cb]: Registered gateway has not sent a heartbeat for more than {} seconds, resetting all internal state", get_check_registered_gateway_delay_ns() / 1_000_000_000);

                    reset_internal_state();

                    v.reset();
                } else {
                    custom_print!("[timer-cb]: Registered gateway is still alive");
                }
            } else {
                custom_print!("[timer-cb]: Registered gateway has not sent a heartbeat yet");
            }
        } else {
            custom_print!("[timer-cb]: No registered gateway");
        }
    });

    schedule_registered_gateway_check();
}

/// Arguments passed to the `on_open` handler.
pub struct OnOpenCallbackArgs {
    pub client_key: ClientPublicKey,
}
/// Handler initialized by the canister and triggered by the CDK once the IC WebSocket connection
/// is established.
type OnOpenCallback = fn(OnOpenCallbackArgs);

/// Arguments passed to the `on_message` handler.
pub struct OnMessageCallbackArgs {
    pub client_key: ClientPublicKey,
    pub message: Vec<u8>,
}
/// Handler initialized by the canister and triggered by the CDK once a message is received by
/// the CDK.
type OnMessageCallback = fn(OnMessageCallbackArgs);

/// Arguments passed to the `on_close` handler.
pub struct OnCloseCallbackArgs {
    pub client_key: ClientPublicKey,
}
/// Handler initialized by the canister and triggered by the CDK once the WS Gateway closes the
/// IC WebSocket connection.
type OnCloseCallback = fn(OnCloseCallbackArgs);

/// Handlers initialized by the canister and triggered by the CDK.
#[derive(Clone)]
pub struct WsHandlers {
    pub on_open: Option<OnOpenCallback>,
    pub on_message: Option<OnMessageCallback>,
    pub on_close: Option<OnCloseCallback>,
}

impl WsHandlers {
    fn call_on_open(&self, args: OnOpenCallbackArgs) {
        if let Some(on_open) = self.on_open {
            on_open(args);
        }
    }

    fn call_on_message(&self, args: OnMessageCallbackArgs) {
        if let Some(on_message) = self.on_message {
            on_message(args);
        }
    }

    fn call_on_close(&self, args: OnCloseCallbackArgs) {
        if let Some(on_close) = self.on_close {
            on_close(args);
        }
    }
}

fn initialize_handlers(handlers: WsHandlers) {
    HANDLERS.with(|h| {
        let mut h = h.borrow_mut();
        *h = handlers;
    });
}

/// Checks the content signature
fn check_content_signature(
    client_key: &ClientPublicKey,
    content: &Vec<u8>,
    sig: &Vec<u8>,
) -> Result<(), String> {
    // check if client_key is a Ed25519 public key
    let public_key = PublicKey::from_slice(client_key).map_err(|e| e.to_string())?;
    // check if the signature relayed by the WS Gateway is a Ed25519 signature
    let sig = Signature::from_slice(sig).map_err(|e| e.to_string())?;
    // check if the signature on the first message verifies against the public key of the registered client
    // if so, the first message came from the same client that registered its public key using ws_register
    public_key.verify(content, &sig).map_err(|e| e.to_string())
}

/// Initialize the CDK by setting the callback handlers and the **principal** of the WS Gateway that
/// will be polling the canister.
pub fn init(handlers: WsHandlers, gateway_principal: &str) {
    // set the handlers specified by the canister that the CDK uses to manage the IC WebSocket connection
    initialize_handlers(handlers);

    // set the principal of the (only) WS Gateway that will be polling the canister
    initialize_registered_gateway(gateway_principal);

    // schedule a timer that will check if the registered gateway is still alive
    schedule_registered_gateway_check();
}

/// Handles the register event received from the client.
///
/// Registers the public key that the client SDK has generated to initialize an IcWebSocket connection.
pub fn ws_register(args: CanisterWsRegisterArguments) -> CanisterWsRegisterResult {
    // TODO: check who is the caller, which can be a client or the anonymous principal

    // associate the identity of the client to its public key received as input
    put_client_caller(args.client_key, caller());
    Ok(())
}

/// Handles the WS connection open event received from the WS Gateway
///
/// WS Gateway relays the first message sent by the client together with its signature
/// to prove that the first message is actually coming from the same client that registered its public key
/// beforehand by calling the [ws_register] method.
pub fn ws_open(args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
    // the caller must be the gateway that was registered during CDK initialization
    check_is_registered_gateway(caller())?;

    // decode the first message sent by the client
    let CanisterOpenMessageContent {
        client_key,
        canister_id,
    } = from_slice(&args.content).map_err(|e| e.to_string())?;

    // check if client registered its public key by calling ws_register
    check_registered_client_key(&client_key)?;

    // check if the signature on the first message verifies against the public key of the registered client
    check_content_signature(&client_key, &args.content, &args.sig)?;

    // initialize client maps
    add_client(client_key.clone());

    Ok(CanisterWsOpenResultValue {
        client_key,
        canister_id,
        // returns the current nonce so that in case the WS Gateway has to open a new poller for this canister
        // it knows which nonce to start polling from. This is needed in order to make sure that the WS Gateway
        // does not poll messages it has already relayed when a new it starts polling a canister
        // (which it might have already polled previously with another thread that was closed after the last client disconnected)
        nonce: get_outgoing_message_nonce(),
    })
}

/// Handles the WS connection close event received from the WS Gateway.
pub fn ws_close(args: CanisterWsCloseArguments) -> CanisterWsCloseResult {
    // the caller must be the gateway that was registered during CDK initialization
    check_is_registered_gateway(caller())?;

    // check if client registered its public key by calling ws_register
    check_registered_client_key(&args.client_key)?;

    remove_client(args.client_key.clone());

    HANDLERS.with(|h| {
        h.borrow().call_on_close(OnCloseCallbackArgs {
            client_key: args.client_key,
        });
    });

    Ok(())
}

/// Handles the WS messages received either directly from the client or relayed by the WS Gateway.
pub fn ws_message(args: CanisterWsMessageArguments) -> CanisterWsMessageResult {
    match args.msg {
        // message sent directly from client
        CanisterIncomingMessage::DirectlyFromClient(received_message) => {
            // check if the identity of the caller corresponds to the one registered for the given public key
            let expected_caller = get_client_caller(&received_message.client_key).ok_or(
                String::from("client is not registered, call ws_register first"),
            )?;
            if caller() != expected_caller {
                return Err(String::from(
                    "caller is not the same that registered the public key",
                ));
            }
            // call the on_message handler initialized in init()
            HANDLERS.with(|h| {
                // trigger the on_message handler initialized by canister
                h.borrow().call_on_message(OnMessageCallbackArgs {
                    client_key: received_message.client_key,
                    message: received_message.message,
                });
            });
            Ok(())
        },
        // WS Gateway relays a message from the client
        CanisterIncomingMessage::RelayedByGateway(received_message) => {
            // this message can come only from the registered gateway
            check_is_registered_gateway(caller())?;

            // decode the message sent by the client
            let WebsocketMessage {
                client_key,
                sequence_num,
                timestamp: _timestamp,
                message,
            } = from_slice(&received_message.content).map_err(|e| e.to_string())?;

            // check if client registered its public key by calling ws_register
            check_registered_client_key(&client_key)?;

            // check if the signature on the message verifies against the public key of the registered client
            check_content_signature(
                &client_key,
                &received_message.content,
                &received_message.sig,
            )?;

            let expected_sequence_num = get_expected_incoming_message_from_client_num(&client_key)?;

            // check if the incoming message has the expected sequence number
            if sequence_num == expected_sequence_num {
                // increase the expected sequence number by 1
                increment_expected_incoming_message_from_client_num(&client_key)?;
                // call the on_message handler initialized in init()
                HANDLERS.with(|h| {
                    // trigger the on_message handler initialized by canister
                    // create message to send to client
                    h.borrow().call_on_message(OnMessageCallbackArgs {
                        client_key,
                        message,
                    });
                });
                return Ok(());
            }
            Err(String::from(
                "incoming client's message relayed from WS Gateway does not have the expected sequence number",
            ))
        },
        // WS Gateway notifies the canister of the established IC WebSocket connection
        CanisterIncomingMessage::IcWebSocketEstablished(client_key) => {
            // this message can come only from the registered gateway
            check_is_registered_gateway(caller())?;

            // check if client registered its public key by calling ws_register
            check_registered_client_key(&client_key)?;

            custom_print!("Can start notifying client with key: {:?}", client_key);
            // call the on_open handler
            HANDLERS.with(|h| {
                // trigger the on_open handler initialized by canister
                h.borrow().call_on_open(OnOpenCallbackArgs { client_key });
            });
            Ok(())
        },
        // WS Gateway notifies the canister that it is up and running
        CanisterIncomingMessage::IcWebSocketGatewayStatus(gateway_status) => {
            // this message can come only from the registered gateway
            check_is_registered_gateway(caller())?;

            update_registered_gateway_status_index(gateway_status.status_index)
        },
    }
}

/// Returns messages to the WS Gateway in response of a polling iteration.
pub fn ws_get_messages(args: CanisterWsGetMessagesArguments) -> CanisterWsGetMessagesResult {
    // check if the caller of this method is the WS Gateway that has been set during the initialization of the SDK
    let gateway_principal = caller();
    check_is_registered_gateway(gateway_principal)?;

    get_cert_messages(gateway_principal, args.nonce)
}

/// Sends a message to the client. The message must already be serialized, using a method of your choice, like Candid or CBOR.
///
/// Under the hood, the message is serialized and certified, and then it is added to the queue of messages
/// that the WS Gateway will poll in the next iteration.
pub fn ws_send(client_key: ClientPublicKey, msg_bytes: Vec<u8>) -> CanisterWsSendResult {
    // check if the client is registered
    check_registered_client_key(&client_key)?;

    // get the principal of the gateway that is polling the canister
    let gateway_principal = get_registered_gateway_principal();

    // the nonce in key is used by the WS Gateway to determine the message to start in the polling iteration
    // the key is also passed to the client in order to validate the body of the certified message
    let outgoing_message_nonce = get_outgoing_message_nonce();
    let key = get_message_for_gateway_key(gateway_principal, outgoing_message_nonce);

    // increment the nonce for the next message
    increment_outgoing_message_nonce();

    // increment the sequence number for the next message to the client
    increment_outgoing_message_to_client_num(&client_key)?;

    let websocket_message = WebsocketMessage {
        client_key: client_key.clone(),
        sequence_num: get_outgoing_message_to_client_num(&client_key)?,
        timestamp: get_current_time(),
        message: msg_bytes,
    };

    // CBOR serialize message of type WebsocketMessage
    let content = websocket_message.cbor_serialize()?;

    // certify data
    put_cert_for_message(key.clone(), &content);

    MESSAGES_FOR_GATEWAY.with(|m| {
        // messages in the queue are inserted with contiguous and increasing nonces
        // (from beginning to end of the queue) as ws_send is called sequentially, the nonce
        // is incremented by one in each call, and the message is pushed at the end of the queue
        m.borrow_mut().push_back(CanisterOutputMessage {
            client_key,
            content,
            key,
        });
    });
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;
    use ring::signature::KeyPair;

    mod test_utils {
        use candid::Principal;
        use ic_agent::{identity::BasicIdentity, Identity};
        use ring::signature::{Ed25519KeyPair, KeyPair};

        use crate::{
            get_message_for_gateway_key, CanisterOutputMessage, ClientPublicKey,
            MESSAGES_FOR_GATEWAY,
        };

        pub fn generate_random_key_pair() -> Ed25519KeyPair {
            let rng = ring::rand::SystemRandom::new();
            let key_pair =
                Ed25519KeyPair::generate_pkcs8(&rng).expect("Could not generate a key pair.");
            Ed25519KeyPair::from_pkcs8(key_pair.as_ref()).expect("Could not read the key pair.")
        }

        pub fn generate_random_principal() -> candid::Principal {
            let key_pair = generate_random_key_pair();
            let identity = BasicIdentity::from_key_pair(key_pair);

            // workaround to keep the principal in the version of candid used by the canister
            candid::Principal::from_text(identity.sender().unwrap().to_text()).unwrap()
        }

        pub fn generate_random_public_key() -> Vec<u8> {
            let key_pair = generate_random_key_pair();

            key_pair.public_key().as_ref().to_vec()
        }

        pub fn get_static_principal() -> Principal {
            Principal::from_text("wnkwv-wdqb5-7wlzr-azfpw-5e5n5-dyxrf-uug7x-qxb55-mkmpa-5jqik-tqe")
                .unwrap() // a random static but valid principal
        }

        pub fn add_messages_for_gateway(
            client_key: ClientPublicKey,
            gateway_principal: Principal,
            count: u64,
        ) {
            MESSAGES_FOR_GATEWAY.with(|m| {
                for i in 0..count {
                    m.borrow_mut().push_back(CanisterOutputMessage {
                        client_key: client_key.clone(),
                        key: get_message_for_gateway_key(gateway_principal.clone(), i),
                        content: vec![],
                    });
                }
            });
        }

        pub fn clean_messages_for_gateway() {
            MESSAGES_FOR_GATEWAY.with(|m| m.borrow_mut().clear());
        }
    }

    // we don't need to proptest get_gateway_principal if principal is not set, as it just panics
    #[test]
    #[should_panic = "gateway should be initialized"]
    fn test_get_gateway_principal_not_set() {
        get_registered_gateway_principal();
    }

    #[test]
    fn test_ws_handlers_are_called() {
        struct CustomState {
            is_on_open_called: bool,
            is_on_message_called: bool,
            is_on_close_called: bool,
        }

        impl CustomState {
            fn new() -> Self {
                Self {
                    is_on_open_called: false,
                    is_on_message_called: false,
                    is_on_close_called: false,
                }
            }
        }

        thread_local! {
            static CUSTOM_STATE : RefCell<CustomState> = RefCell::new(CustomState::new());
        }

        let mut handlers = WsHandlers {
            on_open: None,
            on_message: None,
            on_close: None,
        };

        initialize_handlers(handlers);

        HANDLERS.with(|h| {
            let h = h.borrow();

            assert!(h.on_open.is_none());
            assert!(h.on_message.is_none());
            assert!(h.on_close.is_none());

            h.call_on_open(OnOpenCallbackArgs {
                client_key: test_utils::generate_random_public_key(),
            });
            h.call_on_message(OnMessageCallbackArgs {
                client_key: test_utils::generate_random_public_key(),
                message: vec![],
            });
            h.call_on_close(OnCloseCallbackArgs {
                client_key: test_utils::generate_random_public_key(),
            });
        });

        // test that the handlers are not called if they are not initialized
        assert!(!CUSTOM_STATE.with(|h| h.borrow().is_on_open_called));
        assert!(!CUSTOM_STATE.with(|h| h.borrow().is_on_message_called));
        assert!(!CUSTOM_STATE.with(|h| h.borrow().is_on_close_called));

        // initialize handlers
        let on_open = |_| {
            CUSTOM_STATE.with(|h| {
                let mut h = h.borrow_mut();
                h.is_on_open_called = true;
            });
        };
        let on_message = |_| {
            CUSTOM_STATE.with(|h| {
                let mut h = h.borrow_mut();
                h.is_on_message_called = true;
            });
        };
        let on_close = |_| {
            CUSTOM_STATE.with(|h| {
                let mut h = h.borrow_mut();
                h.is_on_close_called = true;
            });
        };

        handlers = WsHandlers {
            on_open: Some(on_open),
            on_message: Some(on_message),
            on_close: Some(on_close),
        };

        initialize_handlers(handlers);

        HANDLERS.with(|h| {
            let h = h.borrow();

            assert!(h.on_open.is_some());
            assert!(h.on_message.is_some());
            assert!(h.on_close.is_some());

            h.call_on_open(OnOpenCallbackArgs {
                client_key: test_utils::generate_random_public_key(),
            });
            h.call_on_message(OnMessageCallbackArgs {
                client_key: test_utils::generate_random_public_key(),
                message: vec![],
            });
            h.call_on_close(OnCloseCallbackArgs {
                client_key: test_utils::generate_random_public_key(),
            });
        });

        // test that the handlers are called if they are initialized
        assert!(CUSTOM_STATE.with(|h| h.borrow().is_on_open_called));
        assert!(CUSTOM_STATE.with(|h| h.borrow().is_on_message_called));
        assert!(CUSTOM_STATE.with(|h| h.borrow().is_on_close_called));
    }

    #[test]
    fn test_is_integration_test() {
        // test
        assert_eq!(is_integration_test(), false);
    }

    #[test]
    fn test_current_time() {
        // test
        assert_eq!(get_current_time(), 0u64);
    }

    #[test]
    fn test_get_check_registered_gateway_delay() {
        // test
        assert_eq!(
            get_check_registered_gateway_delay_ns(),
            CHECK_REGISTERED_GATEWAY_DELAY_NS
        );
    }

    proptest! {
        #[test]
        fn test_initialize_registered_gateway(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
            initialize_registered_gateway(&test_gateway_principal.to_string());

            REGISTERED_GATEWAY.with(|p| {
                let p = p.borrow();
                assert!(p.is_some());
                assert_eq!(
                    p.unwrap(),
                    RegisteredGateway::new(test_gateway_principal)
                );
            });
        }

        #[test]
        fn test_get_outgoing_message_nonce(test_nonce in any::<u64>()) {
            // Set up
            OUTGOING_MESSAGE_NONCE.with(|n| *n.borrow_mut() = test_nonce);

            let actual_nonce = get_outgoing_message_nonce();
            prop_assert_eq!(actual_nonce, test_nonce);
        }

        #[test]
        fn test_increment_outgoing_message_nonce(test_nonce in any::<u64>()) {
            // Set up
            OUTGOING_MESSAGE_NONCE.with(|n| *n.borrow_mut() = test_nonce);

            increment_outgoing_message_nonce();
            prop_assert_eq!(get_outgoing_message_nonce(), test_nonce + 1);
        }

        #[test]
        fn test_get_client_caller(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            let caller_principal = test_utils::generate_random_principal();
            CLIENT_CALLER_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), caller_principal);
            });

            let actual_client_caller = get_client_caller(&test_client_key);
            prop_assert_eq!(actual_client_caller, Some(caller_principal));
            let actual_client_caller = get_client_caller(&test_utils::generate_random_public_key());
            prop_assert_eq!(actual_client_caller, None);
        }

        #[test]
        fn test_put_client_caller(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            let caller_principal = test_utils::generate_random_principal();

            put_client_caller(test_client_key.clone(), caller_principal);

            let actual_client = CLIENT_CALLER_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_client, caller_principal);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, test_client_key);
        }

        #[test]
        fn test_get_gateway_principal(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
            // Set up
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(test_gateway_principal.clone())));

            let actual_gateway_principal = get_registered_gateway_principal();
            prop_assert_eq!(actual_gateway_principal, test_gateway_principal);
        }

        #[test]
        fn test_update_registered_gateway_status_index(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal()), test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            CLIENT_CALLER_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_utils::generate_random_principal());
            });
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(test_client_key.clone());
            });
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(test_gateway_principal.clone())));

            // test with a valid status index
            let _ = update_registered_gateway_status_index(2);
            let actual_status_index = REGISTERED_GATEWAY.with(|p| p.borrow().as_ref().unwrap().last_status_index.clone());
            prop_assert_eq!(actual_status_index, 2);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().second_last_index_clients.get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, test_client_key.clone());

            // test with an invalid status index (behind the current one)
            let actual_result = update_registered_gateway_status_index(1);
            let actual_status_index = REGISTERED_GATEWAY.with(|p| p.borrow().as_ref().unwrap().last_status_index.clone());
            prop_assert_eq!(actual_status_index, 2);
            prop_assert_eq!(actual_result.err(), Some(String::from("Gateway status index is equal to or behind the current one")));

            // test with an invalid status index (equal to the current one)
            let actual_result = update_registered_gateway_status_index(2);
            let actual_status_index = REGISTERED_GATEWAY.with(|p| p.borrow().as_ref().unwrap().last_status_index.clone());
            prop_assert_eq!(actual_status_index, 2);
            prop_assert_eq!(actual_result.err(), Some(String::from("Gateway status index is equal to or behind the current one")));

            // test with a valid status index (greater one)
            let _ = update_registered_gateway_status_index(10);
            let actual_status_index = REGISTERED_GATEWAY.with(|p| p.borrow().as_ref().unwrap().last_status_index.clone());
            prop_assert_eq!(actual_status_index, 10);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().second_last_index_clients.get(&test_client_key).is_none());
            prop_assert!(actual_result);

            // reset the registered gateway
            let new_client_key = test_utils::generate_random_public_key();
            CLIENT_CALLER_MAP.with(|map| {
                map.borrow_mut().insert(new_client_key.clone(), test_utils::generate_random_principal());
            });
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(new_client_key.clone());
            });
            let _ = update_registered_gateway_status_index(0);
            let actual_status_index = REGISTERED_GATEWAY.with(|p| p.borrow().as_ref().unwrap().last_status_index.clone());
            prop_assert_eq!(actual_status_index, 0);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.get(&new_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, new_client_key.clone());
            let actual_result = CLIENT_CALLER_MAP.with(|map| {
                let map = map.borrow();
                map.get(&test_client_key).is_none() && map.get(&new_client_key).is_some()
            });
            prop_assert!(actual_result);
        }

        #[test]
        fn test_check_registered_client_key_empty(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            let actual_result = check_registered_client_key(&test_client_key);
            prop_assert_eq!(actual_result.err(), Some(String::from("client's public key has not been previously registered by client")));
        }

        #[test]
        fn test_check_registered_client_key(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            CLIENT_CALLER_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_utils::generate_random_principal());
            });

            let actual_result = check_registered_client_key(&test_client_key);
            prop_assert!(actual_result.is_ok());
            let actual_result = check_registered_client_key(&test_utils::generate_random_public_key());
            prop_assert_eq!(actual_result.err(), Some(String::from("client's public key has not been previously registered by client")));
        }

        #[test]
        fn test_init_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            init_outgoing_message_to_client_num(test_client_key.clone());

            let actual_result = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, 0);
        }

        #[test]
        fn test_increment_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key()), test_num in any::<u64>()) {
            // Set up
            OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_num);
            });

            let increment_result = increment_outgoing_message_to_client_num(&test_client_key);
            prop_assert!(increment_result.is_ok());

            let actual_result = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, test_num + 1);
        }

        #[test]
        fn test_get_outgoing_message_to_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key()), test_num in any::<u64>()) {
            // Set up
            OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_num);
            });

            let actual_result = get_outgoing_message_to_client_num(&test_client_key);
            prop_assert!(actual_result.is_ok());
            prop_assert_eq!(actual_result.unwrap(), test_num);
        }

        #[test]
        fn test_init_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            init_expected_incoming_message_from_client_num(test_client_key.clone());

            let actual_result = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, 0);
        }

        #[test]
        fn test_get_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key()), test_num in any::<u64>()) {
            // Set up
            INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_num);
            });

            let actual_result = get_expected_incoming_message_from_client_num(&test_client_key);
            prop_assert!(actual_result.is_ok());
            prop_assert_eq!(actual_result.unwrap(), test_num);
        }

        #[test]
        fn test_increment_expected_incoming_message_from_client_num(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key()), test_num in any::<u64>()) {
            // Set up
            INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_num);
            });

            let increment_result = increment_expected_incoming_message_from_client_num(&test_client_key);
            prop_assert!(increment_result.is_ok());

            let actual_result = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, test_num + 1);
        }

        #[test]
        fn test_add_client(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(test_client_key.clone());
            });

            // Test
            add_client(test_client_key.clone());

            let actual_result = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, 0);

            let actual_result = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, 0);

            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.get(&test_client_key).is_none());
            prop_assert!(actual_result);
        }

        #[test]
        fn test_remove_client(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            CLIENT_CALLER_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), test_utils::generate_random_principal());
            });
            INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), 0);
            });
            OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
                map.borrow_mut().insert(test_client_key.clone(), 0);
            });

            remove_client(test_client_key.clone());

            let is_none = CLIENT_CALLER_MAP.with(|map| map.borrow().get(&test_client_key).is_none());
            prop_assert!(is_none);

            let is_none = INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).is_none());
            prop_assert!(is_none);

            let is_none = OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| map.borrow().get(&test_client_key).is_none());
            prop_assert!(is_none);
        }

        #[test]
        fn test_get_message_for_gateway_key(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal()), test_nonce in any::<u64>()) {
            let actual_result = get_message_for_gateway_key(test_gateway_principal.clone(), test_nonce);
            prop_assert_eq!(actual_result, test_gateway_principal.to_string() + "_" + &format!("{:0>20}", test_nonce.to_string()));
        }

        #[test]
        fn test_get_messages_for_gateway_range_empty(messages_count in any::<u64>().prop_map(|c| c % 1000)) {
            // Set up
            let gateway_principal = test_utils::generate_random_principal();
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(gateway_principal.clone())));

            // Test
            // we ask for a random range of messages to check if it always returns the same range for empty messages
            for i in 0..messages_count {
                let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
                prop_assert_eq!(start_index, 0);
                prop_assert_eq!(end_index, 0);
            }
        }

        #[test]
        fn test_get_messages_for_gateway_range_smaller_than_max(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal())) {
            // Set up
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(gateway_principal.clone())));

            let messages_count = 4;
            let test_client_key = test_utils::generate_random_public_key();
            test_utils::add_messages_for_gateway(test_client_key.clone(), gateway_principal, messages_count);

            // Test
            // messages are just 4, so we don't exceed the max number of returned messages
            // add one to test the out of range index
            for i in 0..messages_count + 1 {
                let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
                prop_assert_eq!(start_index, i as usize);
                prop_assert_eq!(end_index, messages_count as usize);
            }

            // Clean up
            test_utils::clean_messages_for_gateway();
        }

        #[test]
        fn test_get_messages_for_gateway_range_larger_than_max(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal())) {
            // Set up
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(gateway_principal.clone())));

            let messages_count: u64 = (2 * MAX_NUMBER_OF_RETURNED_MESSAGES).try_into().unwrap();
            let test_client_key = test_utils::generate_random_public_key();
            test_utils::add_messages_for_gateway(test_client_key.clone(), gateway_principal, messages_count);

            // Test
            // messages are now MAX_NUMBER_OF_RETURNED_MESSAGES
            for i in 0..messages_count + 1 {
                let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
                let expected_end_index = if (i as usize) + MAX_NUMBER_OF_RETURNED_MESSAGES > messages_count as usize {
                    messages_count as usize
                } else {
                    (i as usize) + MAX_NUMBER_OF_RETURNED_MESSAGES
                };
                prop_assert_eq!(start_index, i as usize);
                prop_assert_eq!(end_index, expected_end_index);
            }

            // Clean up
            test_utils::clean_messages_for_gateway();
        }

        #[test]
        fn test_get_messages_for_gateway(gateway_principal in any::<u8>().prop_map(|_| test_utils::get_static_principal()), messages_count in any::<u64>().prop_map(|c| c % 100)) {
            // Set up
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(gateway_principal.clone())));

            let test_client_key = test_utils::generate_random_public_key();
            test_utils::add_messages_for_gateway(test_client_key.clone(), gateway_principal, messages_count);

            // Test
            // add one to test the out of range index
            for i in 0..messages_count + 1 {
                let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, i);
                let messages = get_messages_for_gateway(start_index, end_index);

                // check if the messages returned are the ones we expect
                for (j, message) in messages.iter().enumerate() {
                    let expected_key = get_message_for_gateway_key(gateway_principal.clone(), (start_index + j) as u64);
                    prop_assert_eq!(&message.key, &expected_key);
                }
            }

            // Clean up
            test_utils::clean_messages_for_gateway();
        }

        #[test]
        fn test_check_is_registered_gateway(test_gateway_principal in any::<u8>().prop_map(|_| test_utils::generate_random_principal())) {
            // Set up
            REGISTERED_GATEWAY.with(|p| *p.borrow_mut() = Some(RegisteredGateway::new(test_gateway_principal.clone())));

            let actual_result = check_is_registered_gateway(test_gateway_principal);
            prop_assert!(actual_result.is_ok());

            let other_principal = test_utils::generate_random_principal();
            let actual_result = check_is_registered_gateway(other_principal);
            prop_assert_eq!(actual_result.err(), Some(String::from("caller is not the gateway that has been registered during CDK initialization")));
        }

        #[test]
        fn test_check_content_signature(test_content in any::<Vec<u8>>(), test_key_pair in any::<u8>().prop_map(|_| test_utils::generate_random_key_pair())) {
            let signature = test_key_pair.sign(&test_content);

            // wrong content
            let actual_result = check_content_signature(&test_key_pair.public_key().as_ref().to_vec(), &vec![0], &signature.as_ref().to_vec());
            prop_assert_eq!(actual_result.err(), Some(String::from("Signature doesn't verify")));

            // wrong public key
            let other_key_pair = test_utils::generate_random_key_pair();
            let actual_result = check_content_signature(&other_key_pair.public_key().as_ref().to_vec(), &test_content, &signature.as_ref().to_vec());
            prop_assert_eq!(actual_result.err(), Some(String::from("Signature doesn't verify")));

            // wrong signature
            let other_signature = other_key_pair.sign(&test_content);
            let actual_result = check_content_signature(&test_key_pair.public_key().as_ref().to_vec(), &test_content, &other_signature.as_ref().to_vec());
            prop_assert_eq!(actual_result.err(), Some(String::from("Signature doesn't verify")));

            // correct signature
            let actual_result = check_content_signature(&test_key_pair.public_key().as_ref().to_vec(), &test_content, &signature.as_ref().to_vec());
            prop_assert!(actual_result.is_ok());
        }

        #[test]
        fn test_serialize_websocket_message(test_msg_bytes in any::<Vec<u8>>(), test_sequence_num in any::<u64>(), test_timestamp in any::<u64>()) {
            // TODO: add more tests, in which we check the serialized message
            let websocket_message = WebsocketMessage {
                client_key: test_utils::generate_random_public_key(),
                sequence_num: test_sequence_num,
                timestamp: test_timestamp,
                message: test_msg_bytes,
            };

            let serialized_message = websocket_message.cbor_serialize();
            assert!(serialized_message.is_ok()); // not so useful as a test
        }

        #[test]
        fn test_insert_in_tmp_clients(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            insert_in_tmp_clients(test_client_key.clone());

            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, test_client_key);
        }

        #[test]
        fn test_shift_tmp_clients(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(test_client_key.clone());
            });

            // Test
            // shift and check if the client is still there
            shift_tmp_clients();

            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.get(&test_client_key).is_none());
            prop_assert!(actual_result);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().second_last_index_clients.get(&test_client_key).unwrap().clone());
            prop_assert_eq!(actual_result, test_client_key.clone());

            // add a new client to the last index clients, and check if the old one is still there
            let new_client = test_utils::generate_random_public_key();
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(new_client.clone());
            });

            // shift again and check if the old one is removed
            shift_tmp_clients();

            let actual_result = TMP_CLIENTS.with(|map| map.borrow().second_last_index_clients.get(&new_client).unwrap().clone());
            prop_assert_eq!(actual_result, new_client);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().second_last_index_clients.get(&test_client_key).is_none());
            prop_assert!(actual_result);

            // shift again and check if everything is empty
            shift_tmp_clients();

            let actual_result = TMP_CLIENTS.with(|map| map.borrow().second_last_index_clients.is_empty());
            prop_assert!(actual_result);
            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.is_empty());
            prop_assert!(actual_result);
        }

        #[test]
        fn test_is_client_in_tmp_clients(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(test_client_key.clone());
            });

            // Test
            let actual_result = is_client_in_tmp_clients(&test_client_key);
            prop_assert!(actual_result);

            let actual_result = is_client_in_tmp_clients(&test_utils::generate_random_public_key());
            prop_assert!(!actual_result);
        }

        #[test]
        fn test_remove_client_from_tmp_clients(test_client_key in any::<u8>().prop_map(|_| test_utils::generate_random_public_key())) {
            // Set up
            TMP_CLIENTS.with(|map| {
                map.borrow_mut().last_index_clients.insert(test_client_key.clone());
            });

            // Test
            remove_client_from_tmp_clients(&test_client_key);

            let actual_result = TMP_CLIENTS.with(|map| map.borrow().last_index_clients.get(&test_client_key).is_none());
            prop_assert!(actual_result);
        }
    }
}
