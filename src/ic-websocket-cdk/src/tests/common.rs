use candid::Principal;
use ic_agent::{identity::BasicIdentity, Identity};
use ring::signature::Ed25519KeyPair;

use crate::ClientKey;

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
