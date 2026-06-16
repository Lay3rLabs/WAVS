//! Wave 0 test stub for P2pStatus format (OBS-02).
//! This will be expanded by Plan 03-01 when P2pStatus is updated.

#[cfg(test)]
mod tests {
    use wavs_types::P2pStatus;

    /// OBS-02: P2pStatus serializes with correct field names and format.
    /// Stub -- will be expanded in Plan 03-01 to verify no multiaddr fields.
    #[test]
    fn p2p_status_format() {
        let status = P2pStatus::default();
        let json = serde_json::to_string(&status).expect("serialize P2pStatus");

        // Verify it produces valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");
        assert!(parsed.is_object());

        // Verify key fields exist
        let obj = parsed.as_object().unwrap();
        assert!(obj.contains_key("enabled"), "Missing 'enabled' field");
        assert!(
            obj.contains_key("connected_peers"),
            "Missing 'connected_peers' field"
        );
        assert!(
            obj.contains_key("listen_addresses"),
            "Missing 'listen_addresses' field"
        );
        assert!(
            obj.contains_key("peer_subscriptions"),
            "Missing 'peer_subscriptions' field"
        );

        // OBS-01: peer_subscriptions defaults to empty object
        let subs = obj.get("peer_subscriptions").unwrap();
        assert!(subs.is_object(), "peer_subscriptions should be an object");
        assert!(
            subs.as_object().unwrap().is_empty(),
            "default peer_subscriptions should be empty"
        );
    }
}
