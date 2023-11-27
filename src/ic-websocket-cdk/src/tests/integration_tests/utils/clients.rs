use crate::{ClientKey, GatewayPrincipal};
use candid::Principal;
use ic_agent::{identity::BasicIdentity, Identity};
use lazy_static::lazy_static;
use ring::signature::Ed25519KeyPair;

lazy_static! {
    pub(in crate::tests::integration_tests) static ref CLIENT_1_KEY: ClientKey =
        generate_client_key("pmisz-prtlk-b6oe6-bj4fl-6l5fy-h7c2h-so6i7-jiz2h-bgto7-piqfr-7ae");
    pub(in crate::tests::integration_tests) static ref CLIENT_2_KEY: ClientKey =
        generate_client_key("zuh6g-qnmvg-vky2t-tnob7-h4xoj-ykrcx-jqjpi-cdf3k-23i3i-ykozs-fae");
    /// The gateway registered in the local PocketIc env
    pub(in crate::tests::integration_tests) static ref GATEWAY_1: GatewayPrincipal =
        Principal::from_text("i3gux-m3hwt-5mh2w-t7wwm-fwx5j-6z6ht-hxguo-t4rfw-qp24z-g5ivt-2qe")
            .unwrap();
    pub(in crate::tests::integration_tests) static ref GATEWAY_2: GatewayPrincipal =
        Principal::from_text("trj6m-u7l6v-zilnb-2hl6a-3jfz3-asri5-mkw3k-e2tpo-5emmk-6hqxb-uae")
            .unwrap();
}

fn generate_client_key(client_principal_text: &str) -> ClientKey {
    ClientKey::new(
        Principal::from_text(client_principal_text).unwrap(),
        generate_random_client_nonce(),
    )
}

pub(in crate::tests::integration_tests) fn generate_random_client_key() -> ClientKey {
    let rng = ring::rand::SystemRandom::new();
    let key_pair = Ed25519KeyPair::generate_pkcs8(&rng)
        .unwrap()
        .as_ref()
        .to_vec();
    let identity = BasicIdentity::from_key_pair(Ed25519KeyPair::from_pkcs8(&key_pair).unwrap());

    ClientKey::new(identity.sender().unwrap(), generate_random_client_nonce())
}

pub fn generate_random_client_nonce() -> u64 {
    rand::random()
}
