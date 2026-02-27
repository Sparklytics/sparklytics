use sha2::{Digest, Sha256};
use sparklytics_core::config::AppMode;

/// Generate a new mode-aware API key.
///
/// Returns (raw_key, hash, prefix).
/// Raw key format:
/// - self-hosted: `spk_selfhosted_` + 32 random hex chars
/// - cloud: `spk_live_` + 32 random hex chars
///
/// Prefix: first 20 chars of raw key (for display/storage parity).
pub fn generate_api_key(mode: &AppMode) -> (String, String, String) {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    let random_part = hex::encode(buf);

    let prefix_base = match mode {
        AppMode::SelfHosted => "spk_selfhosted_",
        AppMode::Cloud => "spk_live_",
    };
    let raw_key = format!("{prefix_base}{random_part}");
    let hash = hash_api_key(&raw_key);
    let prefix = raw_key.chars().take(20).collect::<String>();

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

#[cfg(test)]
mod tests {
    use sparklytics_core::config::AppMode;

    use super::generate_api_key;

    #[test]
    fn generate_selfhosted_key_uses_selfhosted_prefix() {
        let (raw_key, hash, prefix) = generate_api_key(&AppMode::SelfHosted);
        assert!(raw_key.starts_with("spk_selfhosted_"));
        assert_eq!(prefix.len(), 20);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn generate_cloud_key_uses_live_prefix() {
        let (raw_key, hash, prefix) = generate_api_key(&AppMode::Cloud);
        assert!(raw_key.starts_with("spk_live_"));
        assert_eq!(prefix.len(), 20);
        assert_eq!(hash.len(), 64);
    }
}
