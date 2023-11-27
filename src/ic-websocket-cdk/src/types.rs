use std::{collections::VecDeque, fmt, panic};

use candid::{decode_one, CandidType, Principal};
#[cfg(not(test))]
use ic_cdk::api::time;
use serde::{Deserialize, Serialize};
use serde_cbor::Serializer;

use crate::{
    custom_print, custom_trap, errors::WsError, DEFAULT_CLIENT_KEEP_ALIVE_TIMEOUT_MS,
    DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES, DEFAULT_SEND_ACK_INTERVAL_MS,
    INITIAL_OUTGOING_MESSAGE_NONCE,
};

pub type ClientPrincipal = Principal;
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug, Hash)]
pub(crate) struct ClientKey {
    pub(crate) client_principal: ClientPrincipal,
    pub(crate) client_nonce: u64,
}

impl ClientKey {
    /// Creates a new instance of ClientKey.
    pub(crate) fn new(client_principal: ClientPrincipal, client_nonce: u64) -> Self {
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
    pub(crate) client_nonce: u64,
    pub(crate) gateway_principal: GatewayPrincipal,
}

/// The arguments for [ws_close].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsCloseArguments {
    pub(crate) client_key: ClientKey,
}

/// The arguments for [ws_message].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsMessageArguments {
    pub(crate) msg: WebsocketMessage,
}

/// The arguments for [ws_get_messages].
#[derive(CandidType, Clone, Deserialize, Serialize, Eq, PartialEq, Debug)]
pub struct CanisterWsGetMessagesArguments {
    pub(crate) nonce: u64,
}

/// Messages exchanged through the WebSocket.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct WebsocketMessage {
    pub(crate) client_key: ClientKey, // The client that the gateway will forward the message to or that sent the message.
    pub(crate) sequence_num: u64, // Both ways, messages should arrive with sequence numbers 0, 1, 2...
    pub(crate) timestamp: u64, // Timestamp of when the message was made for the recipient to inspect.
    pub(crate) is_service_message: bool, // Whether the message is a service message sent by the CDK to the client or vice versa.
    #[serde(with = "serde_bytes")]
    pub(crate) content: Vec<u8>, // Application message encoded in binary.
}

impl WebsocketMessage {
    /// Serializes the message into a Vec<u8>, using CBOR.
    pub(crate) fn cbor_serialize(&self) -> Result<Vec<u8>, String> {
        let mut data = vec![];
        let mut serializer = Serializer::new(&mut data);
        serializer.self_describe().map_err(|e| e.to_string())?;
        self.serialize(&mut serializer).map_err(|e| e.to_string())?;
        Ok(data)
    }
}

/// Element of the list of messages returned to the WS Gateway after polling.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub(crate) struct CanisterOutputMessage {
    pub(crate) client_key: ClientKey, // The client that the gateway will forward the message to or that sent the message.
    pub(crate) key: String,           // Key for certificate verification.
    #[serde(with = "serde_bytes")]
    pub(crate) content: Vec<u8>, // The message to be relayed, that contains the application message.
}

/// List of messages returned to the WS Gateway after polling.
#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct CanisterOutputCertifiedMessages {
    pub(crate) messages: Vec<CanisterOutputMessage>, // List of messages.
    #[serde(with = "serde_bytes")]
    pub(crate) cert: Vec<u8>, // cert+tree constitute the certificate for all returned messages.
    #[serde(with = "serde_bytes")]
    pub(crate) tree: Vec<u8>, // cert+tree constitute the certificate for all returned messages.
}

pub(crate) type GatewayPrincipal = Principal;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// Contains data about the registered WS Gateway.
pub(crate) struct RegisteredGateway {
    /// The queue of the messages that the gateway can poll.
    pub(crate) messages_queue: VecDeque<CanisterOutputMessage>,
    /// Keeps track of the nonce which:
    /// - the WS Gateway uses to specify the first index of the certified messages to be returned when polling
    /// - the client uses as part of the path in the Merkle tree in order to verify the certificate of the messages relayed by the WS Gateway
    pub(crate) outgoing_message_nonce: u64,
}

impl RegisteredGateway {
    /// Creates a new instance of RegisteredGateway.
    pub(crate) fn new() -> Self {
        Self {
            messages_queue: VecDeque::new(),
            outgoing_message_nonce: INITIAL_OUTGOING_MESSAGE_NONCE,
        }
    }

    /// Increments the outgoing message nonce by 1.
    pub(crate) fn increment_nonce(&mut self) {
        self.outgoing_message_nonce += 1;
    }
}

pub(crate) fn get_current_time() -> u64 {
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
pub(crate) struct RegisteredClient {
    pub(crate) last_keep_alive_timestamp: u64,
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
    pub(crate) fn get_last_keep_alive_timestamp(&self) -> u64 {
        self.last_keep_alive_timestamp
    }

    /// Set the last keep alive timestamp to the current time.
    pub(crate) fn update_last_keep_alive_timestamp(&mut self) {
        self.last_keep_alive_timestamp = get_current_time();
    }
}

#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct CanisterOpenMessageContent {
    pub(crate) client_key: ClientKey,
}

#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct CanisterAckMessageContent {
    pub(crate) last_incoming_sequence_num: u64,
}

#[derive(CandidType, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct ClientKeepAliveMessageContent {
    pub(crate) last_incoming_sequence_num: u64,
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
    pub(crate) fn from_candid_bytes(bytes: &[u8]) -> Result<Self, String> {
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WsHandlers {
    pub on_open: Option<OnOpenCallback>,
    pub on_message: Option<OnMessageCallback>,
    pub on_close: Option<OnCloseCallback>,
}

impl WsHandlers {
    pub(crate) fn call_on_open(&self, args: OnOpenCallbackArgs) {
        if let Some(on_open) = self.on_open {
            let res = panic::catch_unwind(|| {
                on_open(args);
            });

            if let Err(e) = res {
                custom_print!("Error calling on_open handler: {:?}", e);
            }
        }
    }

    pub(crate) fn call_on_message(&self, args: OnMessageCallbackArgs) {
        if let Some(on_message) = self.on_message {
            let res = panic::catch_unwind(|| {
                on_message(args);
            });

            if let Err(e) = res {
                custom_print!("Error calling on_message handler: {:?}", e);
            }
        }
    }

    pub(crate) fn call_on_close(&self, args: OnCloseCallbackArgs) {
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
    /// `send_ack_interval_ms` must be greater than `keep_alive_timeout_ms`.
    ///
    /// # Traps
    /// If `send_ack_interval_ms` <= `keep_alive_timeout_ms`.
    pub(crate) fn check_validity(&self) {
        if self.keep_alive_timeout_ms >= self.send_ack_interval_ms {
            custom_trap!("send_ack_interval_ms must be greater than keep_alive_timeout_ms");
        }
    }

    pub fn with_max_number_of_returned_messages(
        mut self,
        max_number_of_returned_messages: usize,
    ) -> Self {
        self.max_number_of_returned_messages = max_number_of_returned_messages;
        self
    }

    pub fn with_send_ack_interval_ms(mut self, send_ack_interval_ms: u64) -> Self {
        self.send_ack_interval_ms = send_ack_interval_ms;
        self
    }

    pub fn with_keep_alive_timeout_ms(mut self, keep_alive_timeout_ms: u64) -> Self {
        self.keep_alive_timeout_ms = keep_alive_timeout_ms;
        self
    }
}

impl Default for WsInitParams {
    fn default() -> Self {
        Self {
            handlers: WsHandlers::default(),
            max_number_of_returned_messages: DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES,
            send_ack_interval_ms: DEFAULT_SEND_ACK_INTERVAL_MS,
            keep_alive_timeout_ms: DEFAULT_CLIENT_KEEP_ALIVE_TIMEOUT_MS,
        }
    }
}
