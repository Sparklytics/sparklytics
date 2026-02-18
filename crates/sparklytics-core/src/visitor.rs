use sha2::{Digest, Sha256};

/// Compute a visitor ID from IP and User-Agent.
///
/// Formula: sha256(salt_epoch + ip + user_agent)[0..8] encoded as 16 hex chars.
///
/// The salt_epoch = floor(unix_utc_timestamp / 86400) rotates daily at midnight UTC.
/// This function is called only to *generate* a new visitor ID (e.g., when localStorage
/// is empty on the client). Existing IDs stored client-side are reused directly and
/// are never recalculated on subsequent requests — so midnight UTC rotation does not
/// break in-progress sessions.
pub fn compute_visitor_id(ip: &str, user_agent: &str) -> String {
    let salt_epoch = chrono::Utc::now().timestamp() / 86400;
    let input = format!("{}{}{}", salt_epoch, ip, user_agent);
    let hash = Sha256::digest(input.as_bytes());
    // First 8 bytes → 16 hex characters.
    hex::encode(&hash[..8])
}

/// Extract the registrable domain from a full referrer URL.
///
/// Returns `None` if referrer is empty or cannot be parsed to a non-empty host.
pub fn extract_referrer_domain(referrer: &str) -> Option<String> {
    if referrer.is_empty() {
        return None;
    }
    // Strip scheme prefix and take everything before the first '/'.
    let stripped = referrer
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let domain = stripped.split('/').next()?;
    if domain.is_empty() {
        None
    } else {
        Some(domain.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visitor_id_is_16_hex_chars() {
        let id = compute_visitor_id("1.2.3.4", "Mozilla/5.0 Chrome/120");
        assert_eq!(id.len(), 16, "visitor ID must be exactly 16 hex characters");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "visitor ID must contain only hex digits"
        );
    }

    #[test]
    fn visitor_id_is_deterministic_within_same_call() {
        // Two calls with the same inputs in the same second should produce the same ID
        // (salt_epoch is coarse — day-level — so it will not change within a test run).
        let id1 = compute_visitor_id("1.2.3.4", "Mozilla/5.0 Chrome/120");
        let id2 = compute_visitor_id("1.2.3.4", "Mozilla/5.0 Chrome/120");
        assert_eq!(id1, id2);
    }

    #[test]
    fn extract_referrer_domain_https() {
        let domain = extract_referrer_domain("https://news.ycombinator.com/item?id=12345");
        assert_eq!(domain.as_deref(), Some("news.ycombinator.com"));
    }

    #[test]
    fn extract_referrer_domain_http() {
        let domain = extract_referrer_domain("http://google.com/search?q=rust");
        assert_eq!(domain.as_deref(), Some("google.com"));
    }

    #[test]
    fn extract_referrer_domain_empty() {
        assert_eq!(extract_referrer_domain(""), None);
    }
}
