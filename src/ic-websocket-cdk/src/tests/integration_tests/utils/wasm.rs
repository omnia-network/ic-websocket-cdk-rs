use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use super::bin_folder_path;

pub fn load_canister_wasm_from_path(path: &PathBuf) -> Vec<u8> {
    let mut file = File::open(&path)
        .unwrap_or_else(|_| panic!("Failed to open file: {}", path.to_str().unwrap()));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("Failed to read file");
    bytes
}

pub fn load_canister_wasm_from_bin(wasm_name: &str) -> Vec<u8> {
    let mut file_path = bin_folder_path();
    file_path.push(wasm_name);

    load_canister_wasm_from_path(&file_path)
}
