use candid::{decode_one, encode_one};

use super::get_current_timestamp_ns;
use crate::{CanisterOutputMessage, ClientKey, WebsocketMessage, WebsocketServiceMessageContent};

pub(crate) fn get_websocket_message_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketMessage {
    decode_websocket_message(&msg.content)
}

pub(crate) fn encode_websocket_service_message_content(
    content: &WebsocketServiceMessageContent,
) -> Vec<u8> {
    encode_one(content).unwrap()
}

pub(crate) fn decode_websocket_service_message_content(
    bytes: &[u8],
) -> WebsocketServiceMessageContent {
    decode_one(bytes).unwrap()
}

pub(crate) fn get_service_message_content_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketServiceMessageContent {
    let websocket_message = get_websocket_message_from_canister_message(msg);
    decode_websocket_service_message_content(&websocket_message.content)
}

pub(crate) fn create_websocket_message(
    client_key: &ClientKey,
    sequence_number: u64,
    content: Option<Vec<u8>>,
    is_service_message: bool,
) -> WebsocketMessage {
    let content = content.unwrap_or(vec![]);

    WebsocketMessage {
        client_key: client_key.clone(),
        sequence_num: sequence_number,
        timestamp: get_current_timestamp_ns(),
        content,
        is_service_message,
    }
}

pub(crate) fn decode_websocket_message(bytes: &[u8]) -> WebsocketMessage {
    serde_cbor::from_slice(bytes).unwrap()
}

pub fn get_polling_nonce_from_message(message: &CanisterOutputMessage) -> u64 {
    message.key.split("_").last().unwrap().parse().unwrap()
}

pub fn get_next_polling_nonce_from_messages(messages: Vec<CanisterOutputMessage>) -> u64 {
    get_polling_nonce_from_message(messages.last().unwrap()) + 1
}
