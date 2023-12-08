use candid::{CandidType, Principal};
use errors::WsError;

use ic_cdk::api::caller;
use serde::Deserialize;

mod errors;
mod state;
mod tests;
mod timers;
mod types;
mod utils;

use state::*;
use timers::*;
#[allow(deprecated)]
pub use types::CanisterWsSendResult;
use types::*;
pub use types::{
    CanisterCloseResult, CanisterSendResult, CanisterWsCloseArguments, CanisterWsCloseResult,
    CanisterWsGetMessagesArguments, CanisterWsGetMessagesResult, CanisterWsMessageArguments,
    CanisterWsMessageResult, CanisterWsOpenArguments, CanisterWsOpenResult, ClientPrincipal,
    OnCloseCallbackArgs, OnMessageCallbackArgs, OnOpenCallbackArgs, WsHandlers, WsInitParams,
};

/// The label used when constructing the certification tree.
const LABEL_WEBSOCKET: &[u8] = b"websocket";

/// The default maximum number of messages returned by [ws_get_messages] at each poll.
const DEFAULT_MAX_NUMBER_OF_RETURNED_MESSAGES: usize = 50;
/// The default interval at which to send acknowledgements to the client.
const DEFAULT_SEND_ACK_INTERVAL_MS: u64 = 300_000; // 5 minutes
/// The default timeout to wait for the client to send a keep alive after receiving an acknowledgement.
const DEFAULT_CLIENT_KEEP_ALIVE_TIMEOUT_MS: u64 = 60_000; // 1 minute

/// The initial nonce for outgoing messages.
const INITIAL_OUTGOING_MESSAGE_NONCE: u64 = 0;
/// The initial sequence number to expect from messages coming from clients.
/// The first message coming from the client will have sequence number `1` because on the client the sequence number is incremented before sending the message.
const INITIAL_CLIENT_SEQUENCE_NUM: u64 = 1;
/// The initial sequence number for outgoing messages.
const INITIAL_CANISTER_SEQUENCE_NUM: u64 = 0;

/// The number of messages to delete from the outgoing messages queue every time a new message is added.
const MESSAGES_TO_DELETE_COUNT: usize = 5;

/// Initialize the CDK.
///
/// **Note**: Restarts the acknowledgement timers under the hood.
///
/// # Traps
/// If the parameters are invalid.
pub fn init(params: WsInitParams) {
    // check if the parameters are valid
    params.check_validity();

    // set the handlers specified by the canister that the CDK uses to manage the IC WebSocket connection
    set_params(params.clone());

    // cancel possibly running timers
    cancel_timers();

    // schedule a timer that will send an acknowledgement message to clients
    schedule_send_ack_to_clients();
}

/// Handles the WS connection open event sent by the client and relayed by the Gateway.
pub fn ws_open(args: CanisterWsOpenArguments) -> CanisterWsOpenResult {
    let caller = caller();
    // anonymous clients cannot open a connection
    caller
        .ne(&Principal::anonymous())
        .then_some(())
        .ok_or_else(|| WsError::AnonymousPrincipalNotAllowed.to_string())?;

    let client_key = ClientKey::new(caller, args.client_nonce);
    // check if client is not registered yet
    // by swapping the result of the check_registered_client_exists function
    check_registered_client_exists(&client_key).map_or(Ok(()), |_| {
        WsError::ClientKeyAlreadyConnected {
            client_key: &client_key,
        }
        .to_string_result()
    })?;

    // check if there's a client already registered with the same principal
    // and remove it if there is
    match get_client_key_from_principal(&client_key.client_principal) {
        Err(_) => {
            // Do nothing
        },
        Ok(old_client_key) => {
            remove_client(&old_client_key, None);
        },
    };

    // initialize client maps
    let new_client = RegisteredClient::new(args.gateway_principal);
    add_client(client_key.clone(), new_client);

    // send the open message
    let open_message = CanisterOpenMessageContent {
        client_key: client_key.clone(),
    };
    let message = WebsocketServiceMessageContent::OpenMessage(open_message);
    send_service_message_to_client(&client_key, &message)?;

    // call the on_open handler initialized in init()
    get_handlers_from_params().call_on_open(OnOpenCallbackArgs {
        client_principal: client_key.client_principal,
    });

    Ok(())
}

/// Handles the WS connection close event received from the WS Gateway.
///
/// If you want to close the connection with the client in your logic,
/// use the [close] function instead.
pub fn ws_close(args: CanisterWsCloseArguments) -> CanisterWsCloseResult {
    let gateway_principal = caller();

    // check if the gateway is registered
    check_is_gateway_registered(&gateway_principal)?;

    // check if client registered itself by calling ws_open
    check_registered_client_exists(&args.client_key)?;

    // check if the client is registered to the gateway that is closing the connection
    check_client_registered_to_gateway(&args.client_key, &gateway_principal)?;

    remove_client(&args.client_key, None);

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
    let registered_client_key = get_client_key_from_principal(&client_principal)?;

    let WebsocketMessage {
        client_key,
        sequence_num,
        timestamp: _,
        is_service_message,
        content,
    } = args.msg;

    // check if the client key is correct
    client_key
        .eq(&registered_client_key)
        .then_some(())
        .ok_or_else(|| {
            WsError::ClientKeyMessageMismatch {
                client_key: &client_key,
            }
            .to_string()
        })?;

    let expected_sequence_num = get_expected_incoming_message_from_client_num(&client_key)?;

    // check if the incoming message has the expected sequence number
    sequence_num
        .eq(&expected_sequence_num)
        .then_some(())
        .ok_or_else(|| {
            remove_client(&client_key, Some(CloseMessageReason::WrongSequenceNumber));

            WsError::IncomingSequenceNumberWrong {
                expected_sequence_num,
                actual_sequence_num: sequence_num,
            }
            .to_string()
        })?;
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
    let gateway_principal = caller();
    if !is_registered_gateway(&gateway_principal) {
        return get_cert_messages_empty();
    }

    get_cert_messages(&gateway_principal, args.nonce)
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
/// use ic_websocket_cdk::send;
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
/// send(my_client_principal, msg_bytes);
/// ```
pub fn send(client_principal: ClientPrincipal, msg_bytes: Vec<u8>) -> CanisterSendResult {
    let client_key = get_client_key_from_principal(&client_principal)?;
    _ws_send(&client_key, msg_bytes, false)
}

#[deprecated(since = "0.3.2", note = "use `ic_websocket_cdk::send` instead")]
#[allow(deprecated)]
/// Deprecated: use [send] instead.
pub fn ws_send(client_principal: ClientPrincipal, msg_bytes: Vec<u8>) -> CanisterWsSendResult {
    send(client_principal, msg_bytes)
}

/// Closes the connection with the client.
pub fn close(client_principal: ClientPrincipal) -> CanisterCloseResult {
    let client_key = get_client_key_from_principal(&client_principal)?;

    remove_client(&client_key, Some(CloseMessageReason::ClosedByApplication));

    Ok(())
}

/// Resets the internal state of the IC WebSocket CDK.
///
/// **Note:** You should only call this function in tests.
pub fn wipe() {
    reset_internal_state();

    custom_print!("Internal state has been wiped!");
}
