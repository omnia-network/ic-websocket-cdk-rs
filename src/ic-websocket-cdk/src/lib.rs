use candid::{decode_one, encode_one, CandidType, Principal};
#[cfg(not(test))]
use ic_cdk::api::time;
use ic_cdk::api::{caller, data_certificate, set_certified_data};
use ic_cdk::trap;
use ic_cdk_timers::{clear_timer, set_timer, set_timer_interval, TimerId};
use ic_certified_map::{labeled, labeled_hash, AsHashTree, Hash as ICHash, RbTree};
use serde::{Deserialize, Serialize};
use serde_cbor::Serializer;
use sha2::{Digest, Sha256};
use std::fmt;
use std::panic;
use std::rc::Rc;
use std::time::Duration;
use std::{
    cell::RefCell,
    collections::VecDeque,
    collections::{HashMap, HashSet},
    convert::AsRef,
};

mod logger;
mod tests;

/// The label used when constructing the certification tree.
const LABEL_WEBSOCKET: &[u8] = b"websocket";
/// The default maximum number of messages returned by [ws_get_messages] at each poll.
const DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES: usize = 10;
/// The default interval at which to send acknowledgements to the client.
const DEFAULT_SEND_ACK_INTERVAL_MS: u64 = 60_000; // 60 seconds
/// The default timeout to wait for the client to send a keep alive after receiving an acknowledgement.
const DEFAULT_CLIENT_KEEP_ALIVE_TIMEOUT_MS: u64 = 10_000; // 10 seconds

/// The initial nonce for outgoing messages.
const INITIAL_OUTGOING_MESSAGE_NONCE: u64 = 0;
/// The initial sequence number to expect from messages coming from clients.
/// The first message coming from the client will have sequence number `1` because on the client the sequence number is incremented before sending the message.
const INITIAL_CLIENT_SEQUENCE_NUM: u64 = 1;
/// The initial sequence number for outgoing messages.
const INITIAL_CANISTER_SEQUENCE_NUM: u64 = 0;

pub type ClientPrincipal = Principal;
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug, Hash)]
pub(crate) struct ClientKey {
    client_principal: ClientPrincipal,
    client_nonce: u64,
}

impl ClientKey {
    /// Creates a new instance of ClientKey.
    fn new(client_principal: ClientPrincipal, client_nonce: u64) -> Self {
        Self {
            client_principal,
            client_nonce,
        }
    }
}

impl fmt::Display for ClientKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}", self.client_principal, self.client_nonce)
    }
}

/// The result of [ws_open].
pub type CanisterWsOpenResult = Result<(), String>;
/// The result of [ws_close].
pub type CanisterWsCloseResult = Result<(), String>;
/// The result of [ws_message].
pub type CanisterWsMessageResult = Result<(), String>;
/// The result of [ws_get_messages].
pub type CanisterWsGetMessagesResult = Result<CanisterOutputCertifiedMessages, String>;
/// The result of [ws_send].
pub type CanisterWsSendResult = Result<(), String>;

/// The arguments for [ws_open].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsOpenArguments {
    client_nonce: u64,
    gateway_principal: Principal,
}

/// The arguments for [ws_close].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsCloseArguments {
    client_key: ClientKey,
}

/// The arguments for [ws_message].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsMessageArguments {
    msg: WebsocketMessage,
}

/// The arguments for [ws_get_messages].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsGetMessagesArguments {
    nonce: u64,
}

/// Messages exchanged through the WebSocket.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct WebsocketMessage {
    client_key: ClientKey, // The client that the gateway will forward the message to or that sent the message.
    sequence_num: u64,     // Both ways, messages should arrive with sequence numbers 0, 1, 2...
    timestamp: u64,        // Timestamp of when the message was made for the recipient to inspect.
    is_service_message: bool, // Whether the message is a service message sent by the CDK to the client or vice versa.
    #[serde(with = "serde_bytes")]
    content: Vec<u8>, // Application message encoded in binary.
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
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanisterOutputMessage {
    client_key: ClientKey, // The client that the gateway will forward the message to or that sent the message.
    key: String,           // Key for certificate verification.
    #[serde(with = "serde_bytes")]
    content: Vec<u8>, // The message to be relayed, that contains the application message.
}

/// List of messages returned to the WS Gateway after polling.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
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
}

impl RegisteredGateway {
    /// Creates a new instance of RegisteredGateway.
    fn new(gateway_principal: Principal) -> Self {
        Self { gateway_principal }
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

/// The metadata about a registered client.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RegisteredClient {
    last_keep_alive_timestamp: u64,
    gateway_principal: Principal,
}

impl RegisteredClient {
    /// Creates a new instance of RegisteredClient.
    fn new(gateway_principal: Principal) -> Self {
        Self {
            last_keep_alive_timestamp: get_current_time(),
            gateway_principal,
        }
    }

    /// Gets the last keep alive timestamp.
    fn get_last_keep_alive_timestamp(&self) -> u64 {
        self.last_keep_alive_timestamp
    }

    /// Set the last keep alive timestamp to the current time.
    fn update_last_keep_alive_timestamp(&mut self) {
        self.last_keep_alive_timestamp = get_current_time();
    }
}

thread_local! {
    /// Maps the client's key to the client metadata
    /* flexible */ static REGISTERED_CLIENTS: Rc<RefCell<HashMap<ClientKey, RegisteredClient>>> = Rc::new(RefCell::new(HashMap::new()));
    /// Maps the client's principal to the current client key
    /* flexible */ static CURRENT_CLIENT_KEY_MAP: RefCell<HashMap<ClientPrincipal, ClientKey>> = RefCell::new(HashMap::new());
    /// Keeps track of all the clients for which we're waiting for a keep alive message
    /* flexible */ static CLIENTS_WAITING_FOR_KEEP_ALIVE: Rc<RefCell<HashSet<ClientKey>>> = Rc::new(RefCell::new(HashSet::new()));
    /// Maps the client's key to the sequence number to use for the next outgoing message (to that client).
    /* flexible */ static OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP: RefCell<HashMap<ClientKey, u64>> = RefCell::new(HashMap::new());
    /// Maps the client's key to the expected sequence number of the next incoming message (from that client).
    /* flexible */ static INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP: RefCell<HashMap<ClientKey, u64>> = RefCell::new(HashMap::new());
    /// Keeps track of the Merkle tree used for certified queries
    /* flexible */ static CERT_TREE: RefCell<RbTree<String, ICHash>> = RefCell::new(RbTree::new());
    /// Keeps track of the principal of the WS Gateway which polls the canister
    /* flexible */ static REGISTERED_GATEWAYS: RefCell<Option<Vec<RegisteredGateway>>> = RefCell::new(None);
    /// Keeps track of the messages that have to be sent to each authorized WS Gateway
    /* flexible */ static MESSAGES_FOR_GATEWAYS: RefCell<HashMap<Principal, VecDeque<CanisterOutputMessage>>> = RefCell::new(HashMap::new());
    /// Keeps track of the nonce which:
    /// - the WS Gateway uses to specify the first index of the certified messages to be returned when polling
    /// - the client uses as part of the path in the Merkle tree in order to verify the certificate of the messages relayed by the WS Gateway
    /* flexible */ static OUTGOING_MESSAGE_NONCE: RefCell<HashMap<Principal, u64>> = RefCell::new(HashMap::new());
    /// The parameters passed in the CDK initialization
    /* flexible */ static PARAMS: RefCell<WsInitParams> = RefCell::new(WsInitParams::default());
    /// The acknowledgement active timer.
    /* flexible */ static ACK_TIMER: Rc<RefCell<Option<TimerId>>> = Rc::new(RefCell::new(None));
    /// The keep alive active timer.
    /* flexible */ static KEEP_ALIVE_TIMER: Rc<RefCell<Option<TimerId>>> = Rc::new(RefCell::new(None));
}

/// Resets all RefCells to their initial state.
fn reset_internal_state() {
    let client_keys_to_remove: Vec<ClientKey> = REGISTERED_CLIENTS.with(|state| {
        let map = state.borrow();
        map.keys().cloned().collect()
    });

    // for each client, call the on_close handler before clearing the map
    for client_key in client_keys_to_remove {
        remove_client(&client_key);
    }

    // make sure all the maps are cleared
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow_mut().clear();
    });
    CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|set| {
        set.borrow_mut().clear();
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
    MESSAGES_FOR_GATEWAYS.with(|m| *m.borrow_mut() = HashMap::new());
    OUTGOING_MESSAGE_NONCE.with(|m| *m.borrow_mut() = HashMap::new());
}

/// Resets the internal state of the IC WebSocket CDK.
///
/// **Note:** You should only call this function in tests.
pub fn wipe() {
    reset_internal_state();

    custom_print!("Internal state has been wiped!");
}

fn get_outgoing_message_nonce(gateway_principal: &Principal) -> u64 {
    OUTGOING_MESSAGE_NONCE.with(|n| n.borrow().get(gateway_principal).expect("TODO").clone())
}

fn increment_outgoing_message_nonce(gateway_principal: &Principal) {
    OUTGOING_MESSAGE_NONCE.with(|n| {
        let previous_nonce = *n.borrow().get(gateway_principal).expect("TODO");
        n.borrow_mut()
            .insert(*gateway_principal, previous_nonce + 1);
    });
}

fn insert_client(client_key: ClientKey, new_client: RegisteredClient) {
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow_mut()
            .insert(client_key.client_principal.clone(), client_key.clone());
    });
    REGISTERED_CLIENTS.with(|map| {
        map.borrow_mut().insert(client_key, new_client);
    });
}

fn is_client_registered(client_key: &ClientKey) -> bool {
    REGISTERED_CLIENTS.with(|map| map.borrow().contains_key(client_key))
}

fn get_client_key_from_principal(client_principal: &ClientPrincipal) -> Result<ClientKey, String> {
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow()
            .get(client_principal)
            .cloned()
            .ok_or(String::from(format!(
                "client with principal {} doesn't have an open connection",
                client_principal
            )))
    })
}

fn check_registered_client(client_key: &ClientKey) -> Result<(), String> {
    if !is_client_registered(client_key) {
        return Err(String::from(format!(
            "client with key {} doesn't have an open connection",
            client_key
        )));
    }

    Ok(())
}

fn get_gateway_principal_from_registered_client(
    client_key: &ClientKey,
) -> Result<Principal, String> {
    check_registered_client(client_key)?;
    let gateway_principal = REGISTERED_CLIENTS.with(|map| {
        map.borrow()
            .get(client_key)
            .expect("must be registered")
            .gateway_principal
    });
    Ok(gateway_principal)
}

fn add_client_to_wait_for_keep_alive(client_key: &ClientKey) {
    CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|clients| {
        clients.borrow_mut().insert(client_key.clone());
    });
}

fn initialize_registered_gateways(gateways_principals: Vec<String>) {
    REGISTERED_GATEWAYS.with(|p| {
        let registered_gateways = gateways_principals
            .iter()
            .map(|s| {
                RegisteredGateway::new(Principal::from_text(s).expect("invalid gateway principal"))
            })
            .collect();
        *p.borrow_mut() = Some(registered_gateways);
    });
    OUTGOING_MESSAGE_NONCE.with(|n| {
        for gateway_principal in &gateways_principals {
            n.borrow_mut().insert(
                Principal::from_text(gateway_principal).expect("invalid gateway principal"),
                INITIAL_OUTGOING_MESSAGE_NONCE,
            );
        }
    });
    MESSAGES_FOR_GATEWAYS.with(|n| {
        for gateway_principal in &gateways_principals {
            n.borrow_mut().insert(
                Principal::from_text(gateway_principal).expect("invalid gateway principal"),
                VecDeque::new(),
            );
        }
    });
}

fn get_registered_gateways_principals() -> Vec<Principal> {
    REGISTERED_GATEWAYS.with(|g| {
        g.borrow()
            .as_ref()
            .expect("gateway should be initialized")
            .iter()
            .map(|registered_gateway| registered_gateway.gateway_principal)
            .collect()
    })
}

fn init_outgoing_message_to_client_num(client_key: ClientKey) {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut()
            .insert(client_key, INITIAL_CANISTER_SEQUENCE_NUM);
    });
}

fn get_outgoing_message_to_client_num(client_key: &ClientKey) -> Result<u64, String> {
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        let map = map.borrow();
        let num = *map.get(client_key).ok_or(String::from(
            "outgoing message to client num not initialized for client",
        ))?;
        Ok(num)
    })
}

fn increment_outgoing_message_to_client_num(client_key: &ClientKey) -> Result<(), String> {
    let num = get_outgoing_message_to_client_num(client_key)?;
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        let mut map = map.borrow_mut();
        map.insert(client_key.clone(), num + 1);
        Ok(())
    })
}

fn init_expected_incoming_message_from_client_num(client_key: ClientKey) {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut()
            .insert(client_key, INITIAL_CLIENT_SEQUENCE_NUM);
    });
}

fn get_expected_incoming_message_from_client_num(client_key: &ClientKey) -> Result<u64, String> {
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        let num = *map.borrow().get(client_key).ok_or(String::from(
            "expected incoming message num not initialized for client",
        ))?;
        Ok(num)
    })
}

fn increment_expected_incoming_message_from_client_num(
    client_key: &ClientKey,
) -> Result<(), String> {
    let num = get_expected_incoming_message_from_client_num(client_key)?;
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        let mut map = map.borrow_mut();
        map.insert(client_key.clone(), num + 1);
        Ok(())
    })
}

fn add_client(client_key: ClientKey, new_client: RegisteredClient) {
    // insert the client in the map
    insert_client(client_key.clone(), new_client);
    // initialize incoming client's message sequence number to 1
    init_expected_incoming_message_from_client_num(client_key.clone());
    // initialize outgoing message sequence number to 0
    init_outgoing_message_to_client_num(client_key);
}

fn remove_client(client_key: &ClientKey) {
    CLIENTS_WAITING_FOR_KEEP_ALIVE.with(|set| {
        set.borrow_mut().remove(client_key);
    });
    CURRENT_CLIENT_KEY_MAP.with(|map| {
        map.borrow_mut().remove(&client_key.client_principal);
    });
    REGISTERED_CLIENTS.with(|map| {
        map.borrow_mut().remove(client_key);
    });
    OUTGOING_MESSAGE_TO_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().remove(client_key);
    });
    INCOMING_MESSAGE_FROM_CLIENT_NUM_MAP.with(|map| {
        map.borrow_mut().remove(client_key);
    });

    let handlers = get_handlers_from_params();
    handlers.call_on_close(OnCloseCallbackArgs {
        client_principal: client_key.client_principal,
    });
}

fn get_message_for_gateway_key(gateway_principal: Principal, nonce: u64) -> String {
    gateway_principal.to_string() + "_" + &format!("{:0>20}", nonce.to_string())
}

fn get_messages_for_gateway_range(gateway_principal: Principal, nonce: u64) -> (usize, usize) {
    let max_number_of_returned_messages = get_params().max_number_of_returned_messages;

    MESSAGES_FOR_GATEWAYS.with(|h| {
        let queue_len = h.borrow().get(&gateway_principal).expect("TODO").len();

        if nonce == 0 && queue_len > 0 {
            // this is the case in which the poller on the gateway restarted
            // the range to return is end:last index and start: max(end - max_number_of_returned_messages, 0)
            let start_index = if queue_len > max_number_of_returned_messages {
                queue_len - max_number_of_returned_messages
            } else {
                0
            };

            return (start_index, queue_len);
        }

        // smallest key used to determine the first message from the queue which has to be returned to the WS Gateway
        let smallest_key = get_message_for_gateway_key(gateway_principal, nonce);
        // partition the queue at the message which has the key with the nonce specified as argument to get_cert_messages
        let start_index = h
            .borrow()
            .get(&gateway_principal)
            .expect("TODO")
            .partition_point(|x| x.key < smallest_key);
        // message at index corresponding to end index is excluded
        let mut end_index = queue_len;
        if end_index - start_index > max_number_of_returned_messages {
            end_index = start_index + max_number_of_returned_messages;
        }
        (start_index, end_index)
    })
}

fn get_messages_for_gateway(
    gateway_principal: Principal,
    start_index: usize,
    end_index: usize,
) -> Vec<CanisterOutputMessage> {
    MESSAGES_FOR_GATEWAYS.with(|h| {
        let mut messages: Vec<CanisterOutputMessage> = Vec::with_capacity(end_index - start_index);
        for index in start_index..end_index {
            messages.push(
                h.borrow()
                    .get(&gateway_principal)
                    .expect("TODO")
                    .get(index)
                    .unwrap()
                    .clone(),
            );
        }
        messages
    })
}

/// Gets the messages in MESSAGES_FOR_GATEWAYS starting from the one with the specified nonce
fn get_cert_messages(gateway_principal: Principal, nonce: u64) -> CanisterWsGetMessagesResult {
    let (start_index, end_index) = get_messages_for_gateway_range(gateway_principal, nonce);
    let messages = get_messages_for_gateway(gateway_principal, start_index, end_index);

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

fn is_registered_gateway(principal: Principal) -> bool {
    get_registered_gateways_principals().contains(&principal)
}

/// Checks if the caller of the method is one of the authorized WS Gateways that have been registered during the initialization of the CDK
fn check_is_registered_gateway(principal: Principal) -> Result<(), String> {
    if !is_registered_gateway(principal) {
        return Err(String::from(
            "caller is not one of the authorized gateways that have been registered during CDK initialization",
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

fn put_ack_timer_id(timer_id: TimerId) {
    ACK_TIMER.with(|timer| timer.borrow_mut().replace(timer_id));
}

fn reset_ack_timer() {
    if let Some(t_id) = ACK_TIMER.with(Rc::clone).borrow_mut().take() {
        clear_timer(t_id);
    }
}

fn put_keep_alive_timer_id(timer_id: TimerId) {
    KEEP_ALIVE_TIMER.with(|timer| timer.borrow_mut().replace(timer_id));
}

fn reset_keep_alive_timer() {
    if let Some(t_id) = KEEP_ALIVE_TIMER.with(Rc::clone).borrow_mut().take() {
        clear_timer(t_id);
    }
}

fn reset_timers() {
    reset_ack_timer();
    reset_keep_alive_timer();
}

fn set_params(params: WsInitParams) {
    PARAMS.with(|state| *state.borrow_mut() = params);
}

fn get_params() -> WsInitParams {
    PARAMS.with(|state| state.borrow().clone())
}

fn get_handlers_from_params() -> WsHandlers {
    get_params().get_handlers()
}

#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct CanisterOpenMessageContent {
    client_key: ClientKey,
}

#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct CanisterAckMessageContent {
    last_incoming_sequence_num: u64,
}

#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct ClientKeepAliveMessageContent {
    last_incoming_sequence_num: u64,
}

/// A service message sent by the CDK to the client or vice versa.
#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) enum WebsocketServiceMessageContent {
    /// Message sent by the **canister** when a client opens a connection.
    OpenMessage(CanisterOpenMessageContent),
    /// Message sent _periodically_ by the **canister** to the client to acknowledge the messages received.
    AckMessage(CanisterAckMessageContent),
    /// Message sent by the **client** in response to an acknowledgement message from the canister.
    KeepAliveMessage(ClientKeepAliveMessageContent),
}

impl WebsocketServiceMessageContent {
    fn from_candid_bytes(bytes: &[u8]) -> Result<Self, String> {
        decode_one(&bytes).map_err(|e| {
            let mut err = String::from("Error decoding service message content: ");
            err.push_str(&e.to_string());
            err
        })
    }
}

fn send_service_message_to_client(
    client_key: &ClientKey,
    message: WebsocketServiceMessageContent,
) -> Result<(), String> {
    let message_bytes = encode_one(&message).unwrap();
    _ws_send(client_key, message_bytes, true)
}

/// Start an interval to send an acknowledgement messages to the clients.
///
/// The interval callback is [send_ack_to_clients_timer_callback]. After the callback is executed,
/// a timer is scheduled to check if the registered clients have sent a keep alive message.
fn schedule_send_ack_to_clients() {
    let ack_interval_ms = get_params().send_ack_interval_ms;
    let timer_id = set_timer_interval(Duration::from_millis(ack_interval_ms), move || {
        send_ack_to_clients_timer_callback();

        schedule_check_keep_alive();
    });

    put_ack_timer_id(timer_id);
}

/// Schedules a timer to check if the clients (only those to which an ack message was sent) have sent a keep alive message
/// after receiving an acknowledgement message.
///
/// The timer callback is [check_keep_alive_timer_callback].
fn schedule_check_keep_alive() {
    let keep_alive_timeout_ms = get_params().keep_alive_timeout_ms;
    let timer_id = set_timer(Duration::from_millis(keep_alive_timeout_ms), move || {
        check_keep_alive_timer_callback(keep_alive_timeout_ms);
    });

    put_keep_alive_timer_id(timer_id);
}

/// Sends an acknowledgement message to the client.
/// The message contains the current incoming message sequence number for that client,
/// so that the client knows that all the messages it sent have been received by the canister.
fn send_ack_to_clients_timer_callback() {
    for client_key in REGISTERED_CLIENTS.with(Rc::clone).borrow().keys() {
        // ignore the error, which shouldn't happen since the client is registered and the sequence number is initialized
        match get_expected_incoming_message_from_client_num(client_key) {
            Ok(expected_incoming_sequence_num) => {
                let ack_message = CanisterAckMessageContent {
                    // the expected sequence number is 1 more because it's incremented when a message is received
                    last_incoming_sequence_num: expected_incoming_sequence_num - 1,
                };
                let message = WebsocketServiceMessageContent::AckMessage(ack_message);
                if let Err(e) = send_service_message_to_client(client_key, message) {
                    // TODO: decide what to do when sending the message fails

                    custom_print!(
                        "[ack-to-clients-timer-cb]: Error sending ack message to client {}: {:?}",
                        client_key,
                        e
                    );
                } else {
                    add_client_to_wait_for_keep_alive(client_key);
                }
            },
            Err(e) => {
                // TODO: decide what to do when getting the expected incoming sequence number fails (shouldn't happen)
                custom_print!(
                    "[ack-to-clients-timer-cb]: Error getting expected incoming sequence number for client {}: {:?}",
                    client_key,
                    e,
                );
            },
        }
    }

    custom_print!("[ack-to-clients-timer-cb]: Sent ack messages to all clients");
}

/// Checks if the clients for which we are waiting for keep alive have sent a keep alive message.
/// If a client has not sent a keep alive message, it is removed from the connected clients.
fn check_keep_alive_timer_callback(keep_alive_timeout_ms: u64) {
    let client_keys_to_remove: Vec<ClientKey> = CLIENTS_WAITING_FOR_KEEP_ALIVE
        .with(Rc::clone)
        .borrow()
        .iter()
        .filter_map(|client_key| {
            // get the last keep alive timestamp for the client and check if it has exceeded the timeout
            if let Some(client_metadata) =
                REGISTERED_CLIENTS.with(Rc::clone).borrow().get(client_key)
            {
                let last_keep_alive = client_metadata.get_last_keep_alive_timestamp();
                if get_current_time() - last_keep_alive > (keep_alive_timeout_ms * 1_000_000) {
                    Some(client_key.to_owned())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    for client_key in client_keys_to_remove {
        remove_client(&client_key);

        custom_print!(
            "[check-keep-alive-timer-cb]: Client {} has not sent a keep alive message in the last {} ms and has been removed",
            client_key,
            keep_alive_timeout_ms
        );
    }

    custom_print!("[check-keep-alive-timer-cb]: Checked keep alive messages for all clients");
}

fn handle_keep_alive_client_message(
    client_key: &ClientKey,
    _keep_alive_message: ClientKeepAliveMessageContent,
) -> Result<(), String> {
    // TODO: delete messages from the queue that have been acknowledged by the client

    // update the last keep alive timestamp for the client
    if let Some(client_metadata) = REGISTERED_CLIENTS
        .with(Rc::clone)
        .borrow_mut()
        .get_mut(client_key)
    {
        client_metadata.update_last_keep_alive_timestamp();
    }

    Ok(())
}

/// Internal function used to put the messages in the outgoing messages queue and certify them.
fn _ws_send(
    client_key: &ClientKey,
    msg_bytes: Vec<u8>,
    is_service_message: bool,
) -> CanisterWsSendResult {
    // get the principal of the gateway that the client is connected to
    let gateway_principal = get_gateway_principal_from_registered_client(client_key)?;

    // the nonce in key is used by the WS Gateway to determine the message to start in the polling iteration
    // the key is also passed to the client in order to validate the body of the certified message
    let outgoing_message_nonce = get_outgoing_message_nonce(&gateway_principal);
    let key = get_message_for_gateway_key(gateway_principal, outgoing_message_nonce);

    // increment the nonce for the next message
    increment_outgoing_message_nonce(&gateway_principal);
    // increment the sequence number for the next message to the client
    increment_outgoing_message_to_client_num(client_key)?;

    let websocket_message = WebsocketMessage {
        client_key: client_key.clone(),
        sequence_num: get_outgoing_message_to_client_num(client_key)?,
        timestamp: get_current_time(),
        is_service_message,
        content: msg_bytes,
    };

    // CBOR serialize message of type WebsocketMessage
    let content = websocket_message.cbor_serialize()?;

    // certify data
    put_cert_for_message(key.clone(), &content);

    MESSAGES_FOR_GATEWAYS.with(|h| {
        // messages in the queue are inserted with contiguous and increasing nonces
        // (from beginning to end of the queue) as ws_send is called sequentially, the nonce
        // is incremented by one in each call, and the message is pushed at the end of the queue
        h.borrow_mut()
            .get_mut(&gateway_principal)
            .expect("TODO")
            .push_back(CanisterOutputMessage {
                client_key: client_key.clone(),
                content,
                key,
            });
    });
    Ok(())
}

fn handle_received_service_message(
    client_key: &ClientKey,
    content: &[u8],
) -> CanisterWsMessageResult {
    let decoded = WebsocketServiceMessageContent::from_candid_bytes(content)?;
    match decoded {
        WebsocketServiceMessageContent::OpenMessage(_)
        | WebsocketServiceMessageContent::AckMessage(_) => {
            Err(String::from("Invalid received service message"))
        },
        WebsocketServiceMessageContent::KeepAliveMessage(keep_alive_message) => {
            handle_keep_alive_client_message(client_key, keep_alive_message)
        },
    }
}

/// Arguments passed to the `on_open` handler.
pub struct OnOpenCallbackArgs {
    pub client_principal: ClientPrincipal,
}
/// Handler initialized by the canister
/// and triggered by the CDK once the IC WebSocket connection is established.
type OnOpenCallback = fn(OnOpenCallbackArgs);

/// Arguments passed to the `on_message` handler.
/// The `message` argument is the message received from the client, serialized in Candid.
/// To deserialize the message, use [candid::decode_one].
///
/// # Example
/// This example is the deserialize equivalent of the [ws_send's example](fn.ws_send.html#example) serialize one.
/// ```rust
/// use candid::{decode_one, CandidType};
/// use ic_websocket_cdk::OnMessageCallbackArgs;
/// use serde::Deserialize;
///
/// #[derive(CandidType, Deserialize)]
/// struct MyMessage {
///     some_field: String,
/// }
///
/// fn on_message(args: OnMessageCallbackArgs) {
///     let received_message: MyMessage = decode_one(&args.message).unwrap();
///
///     println!("Received message: some_field: {:?}", received_message.some_field);
/// }
/// ```
pub struct OnMessageCallbackArgs {
    /// The principal of the client sending the message to the canister.
    pub client_principal: ClientPrincipal,
    /// The message received from the client, serialized in Candid. See [OnMessageCallbackArgs] for an example on how to deserialize the message.
    pub message: Vec<u8>,
}
/// Handler initialized by the canister
/// and triggered by the CDK once an IC WebSocket message is received.
type OnMessageCallback = fn(OnMessageCallbackArgs);

/// Arguments passed to the `on_close` handler.
pub struct OnCloseCallbackArgs {
    pub client_principal: ClientPrincipal,
}
/// Handler initialized by the canister
/// and triggered by the CDK once the WS Gateway closes the IC WebSocket connection
/// for that client.
type OnCloseCallback = fn(OnCloseCallbackArgs);

/// Handlers initialized by the canister and triggered by the CDK.
#[derive(Clone, Default)]
pub struct WsHandlers {
    pub on_open: Option<OnOpenCallback>,
    pub on_message: Option<OnMessageCallback>,
    pub on_close: Option<OnCloseCallback>,
}

impl WsHandlers {
    fn call_on_open(&self, args: OnOpenCallbackArgs) {
        if let Some(on_open) = self.on_open {
            let res = panic::catch_unwind(|| {
                on_open(args);
            });

            if let Err(e) = res {
                custom_print!("Error calling on_open handler: {:?}", e);
            }
        }
    }

    fn call_on_message(&self, args: OnMessageCallbackArgs) {
        if let Some(on_message) = self.on_message {
            let res = panic::catch_unwind(|| {
                on_message(args);
            });

            if let Err(e) = res {
                custom_print!("Error calling on_message handler: {:?}", e);
            }
        }
    }

    fn call_on_close(&self, args: OnCloseCallbackArgs) {
        if let Some(on_close) = self.on_close {
            let res = panic::catch_unwind(|| {
                on_close(args);
            });

            if let Err(e) = res {
                custom_print!("Error calling on_close handler: {:?}", e);
            }
        }
    }
}

/// Parameters for the IC WebSocket CDK initialization. For default parameters and simpler initialization, use [`WsInitParams::new`].
#[derive(Clone)]
pub struct WsInitParams {
    /// The callback handlers for the WebSocket.
    pub handlers: WsHandlers,
    /// The principals of the WS Gateways that are authorized to poll the canister.
    pub gateway_principals: Vec<String>,
    /// The maximum number of messages to be returned in a polling iteration.
    /// Defaults to `10`.
    pub max_number_of_returned_messages: usize,
    /// The interval at which to send an acknowledgement message to the client,
    /// so that the client knows that all the messages it sent have been received by the canister (in milliseconds).
    ///
    /// Must be greater than `keep_alive_timeout_ms`.
    ///
    /// Defaults to `60_000` (60 seconds).
    pub send_ack_interval_ms: u64,
    /// The delay to wait for the client to send a keep alive after receiving an acknowledgement (in milliseconds).
    ///
    /// Must be lower than `send_ack_interval_ms`.
    ///
    /// Defaults to `10_000` (10 seconds).
    pub keep_alive_timeout_ms: u64,
}

impl WsInitParams {
    /// Creates a new instance of WsInitParams, with default interval values.
    pub fn new(handlers: WsHandlers, gateway_principals: Vec<String>) -> Self {
        Self {
            handlers,
            gateway_principals,
            ..Default::default()
        }
    }

    fn get_handlers(&self) -> WsHandlers {
        self.handlers.clone()
    }

    /// Checks the validity of the timer parameters.
    /// `send_ack_interval_ms` must be greater than `keep_alive_timeout_ms`.
    ///
    /// # Traps
    /// If `send_ack_interval_ms` < `keep_alive_timeout_ms`.
    fn check_validity(&self) {
        if self.keep_alive_timeout_ms > self.send_ack_interval_ms {
            trap("send_ack_interval_ms must be greater than keep_alive_timeout_ms");
        }
    }
}

impl Default for WsInitParams {
    fn default() -> Self {
        Self {
            handlers: WsHandlers::default(),
            gateway_principals: Vec::new(),
            max_number_of_returned_messages: DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES,
            send_ack_interval_ms: DEFAULT_SEND_ACK_INTERVAL_MS,
            keep_alive_timeout_ms: DEFAULT_CLIENT_KEEP_ALIVE_TIMEOUT_MS,
        }
    }
}

/// Initialize the CDK by setting the callback handlers and the **principal** of the WS Gateway that
/// will be polling the canister.
///
/// **Note**: Resets the timers under the hood.
///
/// # Traps
/// If the parameters are invalid. See [`WsInitParams::check_validity`] for more details.
pub fn init(params: WsInitParams) {
    // check if the parameters are valid
    params.check_validity();

    // set the handlers specified by the canister that the CDK uses to manage the IC WebSocket connection
    set_params(params.clone());

    // set the principal of the (only) WS Gateway that will be polling the canister
    initialize_registered_gateways(params.gateway_principals);

    // reset initial timers
    reset_timers();

    // schedule a timer that will send an acknowledgement message to clients
    schedule_send_ack_to_clients();
}

/// Handles the WS connection open event sent by the client and relayed by the Gateway.
pub fn ws_open(args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
    let client_principal = caller();
    // anonymous clients cannot open a connection
    if client_principal == ClientPrincipal::anonymous() {
        return Err(String::from("anonymous principal cannot open a connection"));
    }

    // avoid gateway opening a connection for its own principal
    if is_registered_gateway(client_principal) {
        return Err(String::from(
            "caller is the registered gateway which can't open a connection for itself",
        ));
    }

    let client_key = ClientKey::new(client_principal, args.client_nonce);
    // check if client is not registered yet
    if is_client_registered(&client_key) {
        return Err(format!(
            "client with key {} already has an open connection",
            client_key,
        ));
    }

    // initialize client maps
    let new_client = RegisteredClient::new(args.gateway_principal);
    add_client(client_key.clone(), new_client);

    let open_message = CanisterOpenMessageContent {
        client_key: client_key.clone(),
    };
    let message = WebsocketServiceMessageContent::OpenMessage(open_message);
    send_service_message_to_client(&client_key, message)?;

    // call the on_open handler initialized in init()
    get_handlers_from_params().call_on_open(OnOpenCallbackArgs { client_principal });

    Ok(())
}

/// Handles the WS connection close event received from the WS Gateway.
pub fn ws_close(args: CanisterWsCloseArguments) -> CanisterWsCloseResult {
    // the caller must be the gateway that was registered during CDK initialization
    check_is_registered_gateway(caller())?;

    // check if client registered its principal by calling ws_open
    check_registered_client(&args.client_key)?;

    remove_client(&args.client_key);

    Ok(())
}

/// Handles the WS messages received either directly from the client or relayed by the WS Gateway.
///
/// The second argument is only needed to expose the type of the message on the canister Candid interface and get automatic types generation on the client side.
/// This way, on the client you have the same types and you don't have to care about serializing and deserializing the messages sent through IC WebSocket.
///
/// # Example
/// ```rust
/// use ic_cdk_macros::*;
/// use candid::{CandidType};
/// use ic_websocket_cdk::{CanisterWsMessageArguments, CanisterWsMessageResult};
/// use serde::Deserialize;
///
/// #[derive(CandidType, Deserialize)]
/// struct MyMessage {
///     some_field: String,
/// }
///
/// // method called by the WS Gateway to send a message of type GatewayMessage to the canister
/// #[update]
/// fn ws_message(
///     args: CanisterWsMessageArguments,
///     msg_type: Option<MyMessage>,
/// ) -> CanisterWsMessageResult {
///     ic_websocket_cdk::ws_message(args, msg_type)
/// }
/// ```
pub fn ws_message<T: CandidType + for<'a> Deserialize<'a>>(
    args: CanisterWsMessageArguments,
    _message_type: Option<T>,
) -> CanisterWsMessageResult {
    let client_principal = caller();
    // check if client registered its principal by calling ws_open
    let registered_client_key = get_client_key_from_principal(&client_principal)?;

    let WebsocketMessage {
        client_key,
        sequence_num,
        timestamp: _,
        is_service_message,
        content,
    } = args.msg;

    // check if the client key is correct
    if registered_client_key != client_key {
        return Err(String::from(format!(
            "client with principal {} has a different key than the one used in the message",
            client_principal
        )));
    }

    let expected_sequence_num = get_expected_incoming_message_from_client_num(&client_key)?;

    // check if the incoming message has the expected sequence number
    if sequence_num != expected_sequence_num {
        remove_client(&client_key);
        return Err(String::from(
            format!(
                "incoming client's message does not have the expected sequence number. Expected: {expected_sequence_num}, actual: {sequence_num}. Client removed.",
            ),
        ));
    }
    // increase the expected sequence number by 1
    increment_expected_incoming_message_from_client_num(&client_key)?;

    if is_service_message {
        return handle_received_service_message(&client_key, &content);
    }

    // call the on_message handler initialized in init()
    get_handlers_from_params().call_on_message(OnMessageCallbackArgs {
        client_principal,
        message: content,
    });
    Ok(())
}

/// Returns messages to the WS Gateway in response of a polling iteration.
pub fn ws_get_messages(args: CanisterWsGetMessagesArguments) -> CanisterWsGetMessagesResult {
    // check if the caller of this method is the WS Gateway that has been set during the initialization of the SDK
    let gateway_principal = caller();
    check_is_registered_gateway(gateway_principal)?;

    get_cert_messages(gateway_principal, args.nonce)
}

/// Sends a message to the client. The message must already be serialized **using Candid**.
/// Use [candid::encode_one] to serialize the message.
///
/// Under the hood, the message is certified and added to the queue of messages
/// that the WS Gateway will poll in the next iteration.
///
/// # Example
/// This example is the serialize equivalent of the [OnMessageCallbackArgs's example](struct.OnMessageCallbackArgs.html#example) deserialize one.
/// ```rust
/// use candid::{encode_one, CandidType, Principal};
/// use ic_websocket_cdk::ws_send;
/// use serde::Deserialize;
///
/// #[derive(CandidType, Deserialize)]
/// struct MyMessage {
///     some_field: String,
/// }
///
/// // obtained when the on_open callback was fired
/// let my_client_principal = Principal::from_text("wnkwv-wdqb5-7wlzr-azfpw-5e5n5-dyxrf-uug7x-qxb55-mkmpa-5jqik-tqe").unwrap();
///
/// let my_message = MyMessage {
///     some_field: "Hello, World!".to_string(),
/// };
///
/// let msg_bytes = encode_one(&my_message).unwrap();
/// ws_send(my_client_principal, msg_bytes);
/// ```
pub fn ws_send(client_principal: ClientPrincipal, msg_bytes: Vec<u8>) -> CanisterWsSendResult {
    let client_key = get_client_key_from_principal(&client_principal)?;
    _ws_send(&client_key, msg_bytes, false)
}
