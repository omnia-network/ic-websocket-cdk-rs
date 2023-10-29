use candid::Principal;
use ic_certificate_verification::VerifyCertificate;
use ic_certification::{Certificate, HashTree, LookupResult};
use sha2::{Digest, Sha256};

pub fn is_valid_certificate(
    canister_id: Principal,
    certificate: &[u8],
    tree: &[u8],
    root_ic_key: &[u8],
) -> bool {
    let cert: Certificate = serde_cbor::from_slice(certificate).unwrap();
    let verify_res = cert.verify(canister_id.as_slice(), root_ic_key);
    match verify_res {
        Ok(_) => {
            let tree: HashTree = serde_cbor::from_slice(tree).unwrap();
            match cert.tree.lookup_path(vec![
                b"canister",
                canister_id.as_slice(),
                b"certified_data",
            ]) {
                LookupResult::Found(witness) => witness == tree.digest(),
                _ => return false,
            }
        },
        Err(_) => false,
    }
}

fn hash_body(body: &[u8]) -> [u8; 32] {
    let mut sha = Sha256::new();
    sha.update(body);
    sha.finalize().into()
}

pub fn is_message_body_valid(path: &str, body: &[u8], tree: &[u8]) -> bool {
    let tree: HashTree = serde_cbor::from_slice(tree).unwrap();
    let tree_sha = match tree.lookup_path(vec![b"websocket", path.as_bytes()]) {
        LookupResult::Found(tree_sha) => tree_sha,
        LookupResult::Absent | LookupResult::Unknown => {
            match tree.lookup_path(vec![b"websocket"]) {
                LookupResult::Found(tree_sha) => tree_sha,
                _ => return false,
            }
        },
        LookupResult::Error => return false,
    };
    // sha256 of body
    tree_sha == hash_body(body)
}
