use std::{collections::VecDeque, fmt, time::Duration};

use candid::{decode_one, CandidType, Principal};
use serde::{Deserialize, Serialize};
use serde_cbor::Serializer;

use crate::{
    custom_trap, errors::WsError, utils::get_current_time, CLIENT_KEEP_ALIVE_TIMEOUT_MS,
    DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES, DEFAULT_SEND_ACK_INTERVAL_MS,
    INITIAL_OUTGOING_MESSAGE_NONCE,
};

pub type ClientPrincipal = Principal;
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug, Hash)]
pub struct ClientKey {
    pub client_principal: ClientPrincipal,
    pub client_nonce: u64,
}

impl ClientKey {
    /// Creates a new instance of ClientKey.
    pub fn new(client_principal: ClientPrincipal, client_nonce: u64) -> Self {
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

/// The result of [ws_open](crate::ws_open).
pub type CanisterWsOpenResult = Result<(), String>;
/// The result of [ws_close](crate::ws_close).
pub type CanisterWsCloseResult = Result<(), String>;
/// The result of [ws_message](crate::ws_message).
pub type CanisterWsMessageResult = Result<(), String>;
/// The result of [ws_get_messages](crate::ws_get_messages).
pub type CanisterWsGetMessagesResult = Result<CanisterOutputCertifiedMessages, String>;
/// The result of [send](crate::send).
pub type CanisterSendResult = Result<(), String>;
#[deprecated(since = "0.3.2", note = "use `CanisterSendResult` instead")]
pub type CanisterWsSendResult = Result<(), String>;
/// The result of [close](crate::close).
pub type CanisterCloseResult = Result<(), String>;

/// The arguments for [ws_open](crate::ws_open).
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsOpenArguments {
    pub client_nonce: u64,
    pub gateway_principal: GatewayPrincipal,
}

impl CanisterWsOpenArguments {
    pub fn new(client_nonce: u64, gateway_principal: GatewayPrincipal) -> Self {
        Self {
            client_nonce,
            gateway_principal,
        }
    }
}
/// The arguments for [ws_close](crate::ws_close).
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsCloseArguments {
    pub client_key: ClientKey,
}

impl CanisterWsCloseArguments {
    pub fn new(client_key: ClientKey) -> Self {
        Self { client_key }
    }
}

/// The arguments for [ws_message](crate::ws_message).
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsMessageArguments {
    pub msg: WebsocketMessage,
}

impl CanisterWsMessageArguments {
    pub fn new(msg: WebsocketMessage) -> Self {
        Self { msg }
    }
}

/// The arguments for [ws_get_messages](crate::ws_get_messages).
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsGetMessagesArguments {
    pub nonce: u64,
}

impl CanisterWsGetMessagesArguments {
    pub fn new(nonce: u64) -> Self {
        Self { nonce }
    }
}

/// Messages exchanged through the WebSocket.
///
/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct WebsocketMessage {
    pub client_key: ClientKey, // The client that the gateway will forward the message to or that sent the message.
    pub sequence_num: u64,     // Both ways, messages should arrive with sequence numbers 0, 1, 2...
    pub timestamp: TimestampNs, // Timestamp of when the message was made for the recipient to inspect.
    pub is_service_message: bool, // Whether the message is a service message sent by the CDK to the client or vice versa.
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>, // Application message encoded in binary.
}

impl WebsocketMessage {
    pub fn new(
        client_key: ClientKey,
        sequence_num: u64,
        timestamp: TimestampNs,
        is_service_message: bool,
        content: Vec<u8>,
    ) -> Self {
        Self {
            client_key,
            sequence_num,
            timestamp,
            is_service_message,
            content,
        }
    }

    /// Serializes the message into a Vec<u8>, using CBOR.
    pub fn cbor_serialize(&self) -> Result<Vec<u8>, String> {
        let mut data = vec![];
        let mut serializer = Serializer::new(&mut data);
        serializer.self_describe().map_err(|e| e.to_string())?;
        self.serialize(&mut serializer).map_err(|e| e.to_string())?;
        Ok(data)
    }
}

/// Element of the list of messages returned to the WS Gateway after polling.
///
/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanisterOutputMessage {
    pub client_key: ClientKey, // The client that the gateway will forward the message to or that sent the message.
    pub key: String,           // Key for certificate verification.
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>, // The message to be relayed, that contains the application message.
}

/// List of messages returned to the WS Gateway after polling.
///
/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanisterOutputCertifiedMessages {
    pub messages: Vec<CanisterOutputMessage>, // List of messages.
    #[serde(with = "serde_bytes")]
    pub cert: Vec<u8>, // cert+tree constitute the certificate for all returned messages.
    #[serde(with = "serde_bytes")]
    pub tree: Vec<u8>, // cert+tree constitute the certificate for all returned messages.
    pub is_end_of_queue: bool, // Whether the end of the messages queue has been reached.
}

impl CanisterOutputCertifiedMessages {
    pub fn empty() -> Self {
        Self {
            messages: vec![],
            cert: vec![],
            tree: vec![],
            is_end_of_queue: true,
        }
    }
}

pub(crate) struct MessagesForGatewayRange {
    pub start_index: usize,
    pub end_index: usize,
    pub is_end_of_queue: bool,
}

pub(crate) type TimestampNs = u64;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MessageToDelete {
    timestamp: TimestampNs,
}

pub(crate) type GatewayPrincipal = Principal;

/// Contains data about the registered WS Gateway.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RegisteredGateway {
    /// The queue of the messages that the gateway can poll.
    pub(crate) messages_queue: VecDeque<CanisterOutputMessage>,
    /// The queue of messages' keys to delete.
    pub(crate) messages_to_delete: VecDeque<MessageToDelete>,
    /// Keeps track of the nonce which:
    /// - the WS Gateway uses to specify the first index of the certified messages to be returned when polling
    /// - the client uses as part of the path in the Merkle tree in order to verify the certificate of the messages relayed by the WS Gateway
    pub(crate) outgoing_message_nonce: u64,
    /// The number of clients connected to this gateway.
    pub(crate) connected_clients_count: u64,
}

impl RegisteredGateway {
    /// Creates a new instance of RegisteredGateway.
    pub(crate) fn new() -> Self {
        Self {
            messages_queue: VecDeque::new(),
            messages_to_delete: VecDeque::new(),
            outgoing_message_nonce: INITIAL_OUTGOING_MESSAGE_NONCE,
            connected_clients_count: 0,
        }
    }

    /// Increments the outgoing message nonce by 1.
    pub(crate) fn increment_nonce(&mut self) {
        self.outgoing_message_nonce += 1;
    }

    /// Increments the connected clients count by 1.
    pub(crate) fn increment_clients_count(&mut self) {
        self.connected_clients_count += 1;
    }

    /// Decrements the connected clients count by 1, returning the new value.
    pub(crate) fn decrement_clients_count(&mut self) -> u64 {
        self.connected_clients_count = self.connected_clients_count.saturating_sub(1);
        self.connected_clients_count
    }

    /// Adds the message to the queue and its metadata to the `messages_to_delete` queue.
    pub(crate) fn add_message_to_queue(
        &mut self,
        message: CanisterOutputMessage,
        message_timestamp: TimestampNs,
    ) {
        self.messages_queue.push_back(message.clone());
        self.messages_to_delete.push_back(MessageToDelete {
            timestamp: message_timestamp,
        });
    }

    /// Deletes the oldest `n` messages that are older than `message_max_age_ms` from the queue.
    ///
    /// Returns the deleted messages keys.
    pub(crate) fn delete_old_messages(&mut self, n: usize, message_max_age_ms: u64) -> Vec<String> {
        let time = get_current_time();
        let mut deleted_keys = vec![];

        for _ in 0..n {
            if let Some(message_to_delete) = self.messages_to_delete.front() {
                if Duration::from_nanos(time - message_to_delete.timestamp)
                    > Duration::from_millis(message_max_age_ms)
                {
                    // unwrap is safe because there is no case in which the messages_to_delete queue is populated
                    // while the messages_queue is empty
                    let deleted_message = self.messages_queue.pop_front().unwrap();
                    deleted_keys.push(deleted_message.key.clone());
                    self.messages_to_delete.pop_front();
                } else {
                    // In this case, no messages can be deleted because
                    // they're all not older than `message_max_age_ms`.
                    break;
                }
            } else {
                // There are no messages in the queue. Shouldn't happen.
                break;
            }
        }

        deleted_keys
    }
}

/// The metadata about a registered client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredClient {
    pub(crate) last_keep_alive_timestamp: TimestampNs,
    pub(crate) gateway_principal: GatewayPrincipal,
}

impl RegisteredClient {
    /// Creates a new instance of RegisteredClient.
    pub(crate) fn new(gateway_principal: GatewayPrincipal) -> Self {
        Self {
            last_keep_alive_timestamp: get_current_time(),
            gateway_principal,
        }
    }

    /// Gets the last keep alive timestamp.
    pub(crate) fn get_last_keep_alive_timestamp(&self) -> TimestampNs {
        self.last_keep_alive_timestamp
    }

    /// Set the last keep alive timestamp to the current time.
    pub(crate) fn update_last_keep_alive_timestamp(&mut self) {
        self.last_keep_alive_timestamp = get_current_time();
    }
}

/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub struct CanisterOpenMessageContent {
    pub client_key: ClientKey,
}

/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub struct CanisterAckMessageContent {
    pub last_incoming_sequence_num: u64,
}

/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub struct ClientKeepAliveMessageContent {
    pub last_incoming_sequence_num: u64,
}

/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum CloseMessageReason {
    /// When the canister receives a wrong sequence number from the client.
    WrongSequenceNumber,
    /// When the canister receives an invalid service message from the client.
    InvalidServiceMessage,
    /// When the canister doesn't receive the keep alive message from the client in time.
    KeepAliveTimeout,
    /// When the developer calls the `close` function.
    ClosedByApplication,
}

/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub struct CanisterCloseMessageContent {
    pub reason: CloseMessageReason,
}

/// A service message sent by the CDK to the client or vice versa.
///
/// **Note:** You should only use this struct in tests.
#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub enum WebsocketServiceMessageContent {
    /// Message sent by the **canister** when a client opens a connection.
    OpenMessage(CanisterOpenMessageContent),
    /// Message sent _periodically_ by the **canister** to the client to acknowledge the messages received.
    AckMessage(CanisterAckMessageContent),
    /// Message sent by the **client** in response to an acknowledgement message from the canister.
    KeepAliveMessage(ClientKeepAliveMessageContent),
    /// Message sent by the **canister** when it wants to close the connection.
    CloseMessage(CanisterCloseMessageContent),
}

impl WebsocketServiceMessageContent {
    pub fn from_candid_bytes(bytes: &[u8]) -> Result<Self, String> {
        decode_one(&bytes).map_err(|err| WsError::DecodeServiceMessageContent { err }.to_string())
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
/// This example is the deserialize equivalent of the [send's example](fn.send.html#example) serialize one.
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
///
/// Make sure you **don't** call the [close](crate::close) function in this callback.
type OnCloseCallback = fn(OnCloseCallbackArgs);

/// Handlers initialized by the canister and triggered by the CDK.
///
/// **Note**: if the callbacks that you define here trap for some reason,
/// the CDK will disconnect the client with principal `args.client_principal`.
/// However, the client **won't** be notified
/// until at least the next time it will try to send a message to the canister.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WsHandlers {
    pub on_open: Option<OnOpenCallback>,
    pub on_message: Option<OnMessageCallback>,
    pub on_close: Option<OnCloseCallback>,
}

impl WsHandlers {
    pub(crate) fn call_on_open(&self, args: OnOpenCallbackArgs) {
        if let Some(on_open) = self.on_open {
            // we don't have to recover from errors here,
            // we just let the canister trap
            on_open(args);
        }
    }

    pub(crate) fn call_on_message(&self, args: OnMessageCallbackArgs) {
        if let Some(on_message) = self.on_message {
            // see call_on_open
            on_message(args);
        }
    }

    pub(crate) fn call_on_close(&self, args: OnCloseCallbackArgs) {
        if let Some(on_close) = self.on_close {
            // see call_on_open
            on_close(args);
        }
    }
}

/// Parameters for the IC WebSocket CDK initialization. For default parameters and simpler initialization, use [`WsInitParams::new`].
#[derive(Clone)]
pub struct WsInitParams {
    /// The callback handlers for the WebSocket.
    pub handlers: WsHandlers,
    /// The maximum number of messages to be returned in a polling iteration.
    ///
    /// Defaults to `50`.
    pub max_number_of_returned_messages: usize,
    /// The interval at which to send an acknowledgement message to the client,
    /// so that the client knows that all the messages it sent have been received by the canister (in milliseconds).
    ///
    /// Must be greater than [`CLIENT_KEEP_ALIVE_TIMEOUT_MS`] (1 minute).
    ///
    /// Defaults to `300_000` (5 minutes).
    pub send_ack_interval_ms: u64,
}

impl WsInitParams {
    /// Creates a new instance of WsInitParams, with default interval values.
    pub fn new(handlers: WsHandlers) -> Self {
        Self {
            handlers,
            ..Default::default()
        }
    }

    pub(crate) fn get_handlers(&self) -> WsHandlers {
        self.handlers.clone()
    }

    /// Checks the validity of the timer parameters.
    /// `send_ack_interval_ms` must be greater than [`CLIENT_KEEP_ALIVE_TIMEOUT_MS`].
    ///
    /// # Traps
    /// If `send_ack_interval_ms` <= [`CLIENT_KEEP_ALIVE_TIMEOUT_MS`].
    pub(crate) fn check_validity(&self) {
        if self.send_ack_interval_ms <= CLIENT_KEEP_ALIVE_TIMEOUT_MS {
            custom_trap!("send_ack_interval_ms must be greater than CLIENT_KEEP_ALIVE_TIMEOUT_MS");
        }
    }

    pub fn with_max_number_of_returned_messages(
        mut self,
        max_number_of_returned_messages: usize,
    ) -> Self {
        self.max_number_of_returned_messages = max_number_of_returned_messages;
        self
    }

    /// Sets the interval (in milliseconds) at which to send an acknowledgement message
    /// to the connected clients.
    ///
    /// Must be greater than [`CLIENT_KEEP_ALIVE_TIMEOUT_MS`] (1 minute).
    ///
    /// # Traps
    /// If `send_ack_interval_ms` <= [`CLIENT_KEEP_ALIVE_TIMEOUT_MS`]. See [WsInitParams::check_validity].
    pub fn with_send_ack_interval_ms(mut self, send_ack_interval_ms: u64) -> Self {
        self.send_ack_interval_ms = send_ack_interval_ms;
        self.check_validity();
        self
    }
}

impl Default for WsInitParams {
    fn default() -> Self {
        Self {
            handlers: WsHandlers::default(),
            max_number_of_returned_messages: DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES,
            send_ack_interval_ms: DEFAULT_SEND_ACK_INTERVAL_MS,
        }
    }
}
