use candid::{decode_one, encode_one, CandidType};
use serde::{Deserialize, Serialize};

use super::{
    certification::{is_message_body_valid, is_valid_certificate},
    get_current_timestamp_ns,
    test_env::get_test_env,
};
use crate::{
    types::{CanisterCloseMessageContent, CloseMessageReason},
    CanisterOutputMessage, ClientKey, WebsocketMessage, WebsocketServiceMessageContent,
};

#[derive(CandidType, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AppMessage {
    pub text: String,
}

pub(in crate::tests::integration_tests) fn get_websocket_message_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketMessage {
    decode_websocket_message(&msg.content)
}

pub(in crate::tests::integration_tests) fn encode_websocket_service_message_content(
    content: &WebsocketServiceMessageContent,
) -> Vec<u8> {
    encode_one(content).unwrap()
}

pub(in crate::tests::integration_tests) fn decode_websocket_service_message_content(
    bytes: &[u8],
) -> WebsocketServiceMessageContent {
    decode_one(bytes).unwrap()
}

pub(in crate::tests::integration_tests) fn get_service_message_content_from_canister_message(
    msg: &CanisterOutputMessage,
) -> WebsocketServiceMessageContent {
    let websocket_message = get_websocket_message_from_canister_message(msg);
    decode_websocket_service_message_content(&websocket_message.content)
}

pub(in crate::tests::integration_tests) fn create_websocket_message(
    client_key: &ClientKey,
    sequence_number: u64,
    content: Option<Vec<u8>>,
    is_service_message: bool,
) -> WebsocketMessage {
    let content = content.unwrap_or(vec![]);

    WebsocketMessage::new(
        client_key.clone(),
        sequence_number,
        get_current_timestamp_ns(),
        is_service_message,
        content,
    )
}

pub(in crate::tests::integration_tests) fn decode_websocket_message(
    bytes: &[u8],
) -> WebsocketMessage {
    serde_cbor::from_slice(bytes).unwrap()
}

pub(in crate::tests::integration_tests) fn get_polling_nonce_from_message(
    message: &CanisterOutputMessage,
) -> u64 {
    message.key.split("_").last().unwrap().parse().unwrap()
}

pub(in crate::tests::integration_tests) fn get_next_polling_nonce_from_messages(
    messages: Vec<CanisterOutputMessage>,
) -> u64 {
    get_polling_nonce_from_message(messages.last().unwrap()) + 1
}

pub(in crate::tests::integration_tests) fn verify_messages(
    messages: &Vec<CanisterOutputMessage>,
    client_key: &ClientKey,
    cert: &[u8],
    tree: &[u8],
    expected_sequence_number: &mut u64,
    index: &mut u64,
) {
    for message in messages.iter() {
        verify_message(
            message,
            client_key,
            cert,
            tree,
            *expected_sequence_number,
            *index,
        );

        *expected_sequence_number += 1;
        *index += 1;
    }
}

fn verify_message(
    message: &CanisterOutputMessage,
    client_key: &ClientKey,
    cert: &[u8],
    tree: &[u8],
    expected_sequence_number: u64,
    index: u64,
) {
    assert_eq!(message.client_key, *client_key);
    let websocket_message = decode_websocket_message(&message.content);
    assert_eq!(websocket_message.client_key, *client_key);
    assert_eq!(websocket_message.sequence_num, expected_sequence_number);
    assert!(websocket_message.timestamp <= get_test_env().get_canister_time());

    if websocket_message.is_service_message {
        let decoded_content: WebsocketServiceMessageContent =
            decode_one(&websocket_message.content).unwrap();
        assert!(
            matches!(
                decoded_content,
                WebsocketServiceMessageContent::AckMessage { .. }
            ) || matches!(
                decoded_content,
                WebsocketServiceMessageContent::OpenMessage { .. }
            )
        );
    } else {
        let decoded_content: AppMessage = decode_one(&websocket_message.content).unwrap();
        assert_eq!(
            decoded_content,
            AppMessage {
                text: format!("test{}", index),
            }
        );
    }

    // check the certification
    assert!(is_valid_certificate(&get_test_env(), cert, tree));
    assert!(is_message_body_valid(&message.key, &message.content, tree));
}

pub(in crate::tests::integration_tests) fn check_canister_message_has_close_reason(
    msg: &CanisterOutputMessage,
    close_reason: CloseMessageReason,
) {
    assert_eq!(
        get_service_message_content_from_canister_message(msg),
        WebsocketServiceMessageContent::CloseMessage(CanisterCloseMessageContent {
            reason: close_reason
        }),
    );
}
