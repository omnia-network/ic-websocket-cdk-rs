use candid::decode_one;
use ic_websocket_cdk::{CanisterOutputMessage, WebsocketMessage, WebsocketServiceMessageContent};

pub fn get_websocket_message_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketMessage {
    serde_cbor::from_slice(&msg.content).unwrap()
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
