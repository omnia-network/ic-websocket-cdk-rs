use candid::Principal;
use ic_agent::{identity::BasicIdentity, Identity};
use ring::signature::Ed25519KeyPair;

use crate::ClientKey;

fn generate_random_key_pair() -> Ed25519KeyPair {
    let rng = ring::rand::SystemRandom::new();
    let key_pair = Ed25519KeyPair::generate_pkcs8(&rng).expect("Could not generate a key pair.");
    Ed25519KeyPair::from_pkcs8(key_pair.as_ref()).expect("Could not read the key pair.")
}

pub(super) fn generate_random_principal() -> Principal {
    let key_pair = generate_random_key_pair();
    let identity = BasicIdentity::from_key_pair(key_pair);

    identity.sender().unwrap()
}

pub(super) fn get_random_client_key() -> ClientKey {
    ClientKey::new(
        generate_random_principal(),
        // a random nonce
        rand::random(),
    )
}
