//! Wave 0 test stubs for P2pConfig (CFG-01 and CFG-02).
//! These verify the current P2pConfig shape including max_message_size and deque_size.

#[cfg(test)]
mod tests {
    use super::super::p2p::P2pConfig;

    /// CFG-01: P2pConfig roundtrips through serde correctly and deserializes from TOML.
    #[test]
    fn p2p_config_serde() {
        // Verify Disabled variant deserializes from TOML
        let disabled_toml = r#""disabled""#;
        let parsed: P2pConfig = serde_json::from_str(disabled_toml).expect("deserialize Disabled");
        assert_eq!(parsed, P2pConfig::Disabled);

        // Verify Disabled roundtrips through JSON
        let json_str = serde_json::to_string(&P2pConfig::Disabled).expect("serialize Disabled");
        let parsed: P2pConfig = serde_json::from_str(&json_str).expect("roundtrip Disabled");
        assert_eq!(parsed, P2pConfig::Disabled);

        // Verify Local variant deserializes from TOML
        let local_toml = r#"
            [local]
            listen_port = 9000
            peer_addresses = ["abc123@127.0.0.1:9001"]
            max_message_size = 32768
            deque_size = 256
        "#;
        let parsed: P2pConfig = toml::from_str(local_toml).expect("deserialize Local from TOML");
        assert_eq!(
            parsed,
            P2pConfig::Local {
                listen_port: 9000,
                peer_addresses: vec!["abc123@127.0.0.1:9001".to_string()],
                authorized_peers: vec![],
                max_message_size: Some(32768),
                deque_size: Some(256),
            }
        );

        // Verify Local variant roundtrips through JSON
        let local = P2pConfig::Local {
            listen_port: 9000,
            peer_addresses: vec!["abc123@127.0.0.1:9001".to_string()],
            authorized_peers: vec![],
            max_message_size: Some(32768),
            deque_size: Some(256),
        };
        let json_str = serde_json::to_string(&local).expect("serialize Local");
        let parsed: P2pConfig = serde_json::from_str(&json_str).expect("roundtrip Local");
        assert_eq!(parsed, local);
    }

    /// CFG-02: Default values for optional tuning fields.
    #[test]
    fn p2p_config_defaults() {
        // Default is Disabled
        let default = P2pConfig::default();
        assert_eq!(default, P2pConfig::Disabled);

        // Local variant with only required fields should deserialize (optional fields default)
        let toml_str = r#"
            [local]
            listen_port = 9000
        "#;
        let parsed: P2pConfig = toml::from_str(toml_str).expect("deserialize Local with defaults");
        match &parsed {
            P2pConfig::Local {
                listen_port,
                peer_addresses,
                authorized_peers,
                max_message_size,
                deque_size,
            } => {
                assert_eq!(*listen_port, 9000);
                assert!(peer_addresses.is_empty());
                assert!(authorized_peers.is_empty());
                assert_eq!(*max_message_size, None);
                assert_eq!(*deque_size, None);
            }
            other => panic!("Expected Local, got {:?}", other),
        }

        // Verify helper methods return correct defaults when None
        assert_eq!(parsed.max_message_size(), 65536);
        assert_eq!(parsed.deque_size(), 128);
    }
}
