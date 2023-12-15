use crate::{
    format_message_for_gateway_key, set_params, tests::common::generate_random_principal,
    CanisterOutputMessage, ClientKey, GatewayPrincipal, RegisteredClient, WsInitParams,
    REGISTERED_GATEWAYS,
};

pub(super) fn generate_random_registered_client() -> RegisteredClient {
    RegisteredClient::new(generate_random_principal())
}

pub(super) fn add_messages_for_gateway(
    client_key: ClientKey,
    gateway_principal: &GatewayPrincipal,
    count: u64,
) {
    REGISTERED_GATEWAYS.with(|m| {
        for i in 0..count {
            m.borrow_mut()
                .get_mut(gateway_principal)
                .unwrap()
                .messages_queue
                .push_back(CanisterOutputMessage {
                    client_key: client_key.clone(),
                    key: format_message_for_gateway_key(&gateway_principal, i),
                    content: vec![],
                });
        }
    });
}

pub fn clean_messages_for_gateway(gateway_principal: &GatewayPrincipal) {
    REGISTERED_GATEWAYS.with(|m| {
        m.borrow_mut()
            .get_mut(gateway_principal)
            .unwrap()
            .messages_queue
            .clear()
    });
}

pub fn initialize_params() {
    set_params(WsInitParams {
        ..Default::default()
    });
}

pub fn generate_random_message_key(gateway_principal: &GatewayPrincipal) -> String {
    format_message_for_gateway_key(gateway_principal, rand::random())
}
