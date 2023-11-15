use std::path::PathBuf;

pub mod actor;
pub mod certification;
pub mod clients;
pub mod constants;
pub mod messages;
pub mod test_env;
pub mod wasm;

pub fn bin_folder_path() -> PathBuf {
    println!("manifest dir: {:?}", std::env::var("CARGO_MANIFEST_DIR"));
    let mut file_path = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("Failed to read CARGO_MANIFEST_DIR env variable"),
    );
    file_path.pop();
    file_path.pop();
    file_path.push("bin");
    file_path
}

/// Returns the current timestamp in nanoseconds.
pub fn get_current_timestamp_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
