use std::{error::Error, fmt};

use crate::{ClientKey, ClientPrincipal, GatewayPrincipal};

#[derive(Debug)]
pub(crate) enum WsError<'a> {
    AnonymousPrincipalNotAllowed,
    ClientKeyAlreadyConnected {
        client_key: &'a ClientKey,
    },
    ClientKeyMessageMismatch {
        client_key: &'a ClientKey,
    },
    ClientKeyNotConnected {
        client_key: &'a ClientKey,
    },
    ClientNotRegisteredToGateway {
        client_key: &'a ClientKey,
        gateway_principal: &'a GatewayPrincipal,
    },
    ClientPrincipalNotConnected {
        client_principal: &'a ClientPrincipal,
    },
    DecodeServiceMessageContent {
        err: candid::Error,
    },
    ExpectedIncomingMessageToClientNumNotInitialized {
        client_key: &'a ClientKey,
    },
    GatewayNotRegistered {
        gateway_principal: &'a GatewayPrincipal,
    },
    InvalidServiceMessage,
    IncomingSequenceNumberWrong {
        expected_sequence_num: u64,
        actual_sequence_num: u64,
    },
    OutgoingMessageToClientNumNotInitialized {
        client_key: &'a ClientKey,
    },
}

impl WsError<'_> {
    pub(crate) fn to_string_result(&self) -> Result<(), String> {
        Err(self.to_string())
    }
}

impl fmt::Display for WsError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            WsError::AnonymousPrincipalNotAllowed => {
                "Anonymous principal is not allowed".to_string()
            },
            WsError::ClientKeyAlreadyConnected { client_key } => {
                format!("Client with key {client_key} already has an open connection")
            },
            WsError::ClientKeyMessageMismatch { client_key } => {
                format!(
                    "Client with principal {} has a different key than the one used in the message",
                    client_key.client_principal,
                )
            },
            WsError::ClientKeyNotConnected { client_key } => {
                format!("Client with key {client_key} doesn't have an open connection")
            },
            WsError::ClientNotRegisteredToGateway {
                client_key,
                gateway_principal,
            } => {
                format!(
                    "Client with key {client_key} was not registered to gateway {gateway_principal}"
                )
            },
            WsError::ClientPrincipalNotConnected { client_principal } => {
                format!("Client with principal {client_principal} doesn't have an open connection")
            },
            WsError::DecodeServiceMessageContent { err } => {
                format!("Error decoding service message content: {err}")
            },
            WsError::ExpectedIncomingMessageToClientNumNotInitialized { client_key } => {
                format!(
                    "Expected incoming message to client num not initialized for client key {client_key}"
                )
            },
            WsError::GatewayNotRegistered { gateway_principal } => {
                format!("Gateway with principal {gateway_principal} is not registered")
            },
            WsError::InvalidServiceMessage => "Invalid service message".to_string(),
            WsError::IncomingSequenceNumberWrong {
                expected_sequence_num,
                actual_sequence_num,
            } => {
                format!(
                    "Expected incoming sequence number {expected_sequence_num} but got {actual_sequence_num}"
                )
            },
            WsError::OutgoingMessageToClientNumNotInitialized { client_key } => {
                format!(
                    "Outgoing message to client num not initialized for client key {client_key}"
                )
            },
        };

        write!(f, "{}", msg)
    }
}

impl Error for WsError<'_> {}
