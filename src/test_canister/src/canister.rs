use candid::CandidType;
use ic_cdk::println;

use ic_websocket_cdk::{OnCloseCallbackArgs, OnMessageCallbackArgs, OnOpenCallbackArgs};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize)]
pub struct AppMessage {
    pub text: String,
}

pub fn on_open(args: OnOpenCallbackArgs) {
    println!("Opened websocket: {}", args.client_principal.to_text());
}

pub fn on_message(args: OnMessageCallbackArgs) {
    println!("Received message: {}", args.client_principal.to_text());
}

pub fn on_close(args: OnCloseCallbackArgs) {
    println!("Client {} disconnected", args.client_principal.to_text());
}
