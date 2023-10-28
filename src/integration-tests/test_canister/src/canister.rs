use candid::CandidType;
use ic_cdk::print;

use ic_websocket_cdk::{OnCloseCallbackArgs, OnMessageCallbackArgs, OnOpenCallbackArgs};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize)]
pub struct AppMessage {
    pub text: String,
}

pub fn on_open(args: OnOpenCallbackArgs) {
    print(format!("Opened websocket: {:?}", args.client_principal));
}

pub fn on_message(args: OnMessageCallbackArgs) {
    print(format!("Received message: {:?}", args.client_principal));
}

pub fn on_close(args: OnCloseCallbackArgs) {
    print(format!("Client {:?} disconnected", args.client_principal));
}
