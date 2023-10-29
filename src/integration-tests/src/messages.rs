use candid::{decode_one, encode_one};
use ic_websocket_cdk::{
    CanisterOutputMessage, ClientKey, WebsocketMessage, WebsocketServiceMessageContent,
};

use crate::utils::get_current_timestamp_ns;

pub fn get_websocket_message_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketMessage {
    serde_cbor::from_slice(&msg.content).unwrap()
}

pub fn encode_websocket_service_message_content(
    content: &WebsocketServiceMessageContent,
) -> Vec<u8> {
    encode_one(content).unwrap()
}

pub fn decode_websocket_service_message_content(bytes: &[u8]) -> WebsocketServiceMessageContent {
    decode_one(bytes).unwrap()
}

pub fn get_service_message_content_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketServiceMessageContent {
    let websocket_message = get_websocket_message_from_canister_message(msg);
    decode_websocket_service_message_content(&websocket_message.content)
}

pub fn create_websocket_message(
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
