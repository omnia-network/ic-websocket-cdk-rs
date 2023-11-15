use candid::Principal;
use ic_agent::{identity::BasicIdentity, Identity};
use ring::signature::Ed25519KeyPair;

use crate::{
    format_message_for_gateway_key, set_params, CanisterOutputMessage, ClientKey, GatewayPrincipal,
    RegisteredClient, WsInitParams, REGISTERED_GATEWAYS,
};

fn generate_random_key_pair() -> Ed25519KeyPair {
    let rng = ring::rand::SystemRandom::new();
    let key_pair = Ed25519KeyPair::generate_pkcs8(&rng).expect("Could not generate a key pair.");
    Ed25519KeyPair::from_pkcs8(key_pair.as_ref()).expect("Could not read the key pair.")
}

pub fn generate_random_principal() -> Principal {
    let key_pair = generate_random_key_pair();
    let identity = BasicIdentity::from_key_pair(key_pair);

    // workaround to keep the principal in the version of candid used by the canister
    Principal::from_text(identity.sender().unwrap().to_text()).unwrap()
}

pub(super) fn generate_random_registered_client() -> RegisteredClient {
    RegisteredClient::new(Principal::anonymous())
}

pub fn get_static_principal() -> Principal {
    Principal::from_text("wnkwv-wdqb5-7wlzr-azfpw-5e5n5-dyxrf-uug7x-qxb55-mkmpa-5jqik-tqe").unwrap()
    // a random static but valid principal
}

pub(super) fn get_random_client_key() -> ClientKey {
    ClientKey::new(
        generate_random_principal(),
        // a random nonce
        rand::random(),
    )
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
