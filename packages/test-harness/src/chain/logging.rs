//! Sanitized logging for chain operations.
//!
//! Never log raw RPC URLs or private keys. Use [`redact_url`] / [`redact_key`] anywhere
//! a sensitive value crosses into a log line.

/// Redact an RPC URL without leaking credentials, query strings, or fragments.
///
/// Keeps only scheme, host, and port. Paths, query strings, fragments, and userinfo
/// are removed because provider API keys commonly appear there.
pub fn redact_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("redacted");
            match parsed.port() {
                Some(port) => format!("{scheme}://{host}:{port}/…"),
                None => format!("{scheme}://{host}/…"),
            }
        }
        Err(_) => "<redacted-url>".to_string(),
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
    fn redacts_url_paths_queries_fragments_and_credentials() {
        let s = redact_url("https://user:secret@api.example.com/v1/abc123XYZ?key=leak#frag");
        assert_eq!(s, "https://api.example.com/…");
        assert!(!s.contains("secret"));
        assert!(!s.contains("abc123XYZ"));
        assert!(!s.contains("key="));
        assert!(!s.contains("frag"));
    }

    #[test]
    fn redacts_keys_to_short_prefix() {
        let s = redact_key("0xdeadbeefcafebabe1234567890");
        assert!(s.starts_with("deadbe"));
        assert!(!s.contains("cafebabe"));
    }

    #[test]
    fn redacts_short_inputs_safely() {
        assert_eq!(redact_url("x"), "<redacted-url>");
        assert_eq!(redact_key("0xab"), "<redacted>");
    }
}
