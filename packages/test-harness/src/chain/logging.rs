//! Sanitized logging for chain operations.
//!
//! Never log raw RPC URLs or private keys. Use [`redact_url`] / [`redact_key`] anywhere
//! a sensitive value crosses into a log line.

/// Redact an RPC URL down to a non-identifying suffix.
///
/// Returns `…<last-8-chars>` for strings longer than 8 characters, otherwise `…<input>`.
/// This is enough to disambiguate which provider is in use without leaking the API key.
pub fn redact_url(url: &str) -> String {
    if url.len() > 8 {
        format!("…{}", &url[url.len() - 8..])
    } else {
        format!("…{url}")
    }
}

/// Redact a private key or mnemonic down to a non-identifying prefix.
///
/// Returns `<first-6-chars>…` (after stripping any leading `0x`). For short or empty input,
/// returns a generic placeholder.
pub fn redact_key(key: &str) -> String {
    let trimmed = key.strip_prefix("0x").unwrap_or(key);
    if trimmed.len() >= 6 {
        format!("{}…", &trimmed[..6])
    } else {
        "<redacted>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_long_urls_to_suffix() {
        let s = redact_url("https://api.example.com/v1/abc123XYZ");
        assert_eq!(s, "…bc123XYZ");
        assert!(!s.contains("example"));
    }

    #[test]
    fn redacts_keys_to_short_prefix() {
        let s = redact_key("0xdeadbeefcafebabe1234567890");
        assert!(s.starts_with("deadbe"));
        assert!(!s.contains("cafebabe"));
    }

    #[test]
    fn redacts_short_inputs_safely() {
        assert_eq!(redact_url("x"), "…x");
        assert_eq!(redact_key("0xab"), "<redacted>");
    }
}
