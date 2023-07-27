use ic_cdk::{export::candid::CandidType, print};
use serde::{Deserialize, Serialize};

use ic_websocket_cdk::{OnCloseCallbackArgs, OnMessageCallbackArgs, OnOpenCallbackArgs};

pub const GATEWAY_PRINCIPAL: &str =
    "sqdfl-mr4km-2hfjy-gajqo-xqvh7-hf4mf-nra4i-3it6l-neaw4-soolw-tae";

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[candid_path("ic_cdk::export::candid")]
pub struct AppMessage {
    pub text: String,
}

pub fn on_open(args: OnOpenCallbackArgs) {
    print(format!("Opened websocket: {:?}", args.client_key));
}

pub fn on_message(args: OnMessageCallbackArgs) {
    print(format!("Received message: {:?}", args.client_key));
}

pub fn on_close(args: OnCloseCallbackArgs) {
    print(format!("Client {:?} disconnected", args.client_key));
}
