use sha2::{Digest, Sha256};

/// Generate a new self-hosted API key.
///
/// Returns (raw_key, hash, prefix).
/// Raw key format: `spk_selfhosted_` + 32 random hex chars.
/// Prefix: first 25 chars of raw key (for display).
pub fn generate_api_key() -> (String, String, String) {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    let random_part = hex::encode(buf);

    let raw_key = format!("spk_selfhosted_{}", random_part);
    let hash = hash_api_key(&raw_key);
    let prefix = raw_key[..25].to_string();

    (raw_key, hash, prefix)
}

/// Generate a key ID: "key_" + 10 random alphanumeric chars.
pub fn generate_key_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: String = (0..10)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("key_{}", chars)
}

/// Hash an API key with SHA-256.
pub fn hash_api_key(raw_key: &str) -> String {
    let hash = Sha256::digest(raw_key.as_bytes());
    hex::encode(hash)
}
