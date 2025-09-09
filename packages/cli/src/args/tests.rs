use super::*;
use clap::Parser;

// Helper function to validate config key=value format
fn validate_config_format(config: &str) {
    let parts: Vec<&str> = config.splitn(2, '=').collect();
    assert_eq!(
        parts.len(),
        2,
        "Config should have key=value format: {}",
        config
    );
    assert!(!parts[0].is_empty(), "Key should not be empty: {}", config);
}

// Helper function to parse command and extract config for testing
fn parse_command_config(cmd_args: Vec<&str>) -> Option<Vec<String>> {
    let parsed = Command::try_parse_from(cmd_args).unwrap();
    match parsed {
        Command::Exec { config, .. } => Some(config),
        Command::ExecAggregator { config, .. } => config,
        _ => None,
    }
}

#[test]
fn test_exec_commands_parsing() {
    // Test cases for both exec and exec-aggregator commands
    let test_cases = vec![
        (
            vec![
                "test",
                "exec",
                "--component",
                "test.wasm",
                "--input",
                "test",
                "--config",
                "key=value",
            ],
            "exec",
        ),
        (
            vec![
                "test",
                "exec-aggregator",
                "--component",
                "test.wasm",
                "--config",
                "chain=evm:31337",
            ],
            "exec-aggregator",
        ),
    ];

    for (args, expected_cmd) in test_cases {
        let result = Command::try_parse_from(args.clone()).unwrap();
        match (result, expected_cmd) {
            (
                Command::Exec {
                    component, config, ..
                },
                "exec",
            ) => {
                assert_eq!(component, "test.wasm");
                assert!(config.contains(&"key=value".to_string()));
            }
            (
                Command::ExecAggregator {
                    component, config, ..
                },
                "exec-aggregator",
            ) => {
                assert_eq!(component, "test.wasm");
                assert!(config.is_some());
                let config_vec = config.unwrap();
                assert!(config_vec.contains(&"chain=evm:31337".to_string()));
            }
            _ => panic!("Unexpected command type for test case: {:?}", args),
        }
    }
}

#[test]
fn test_config_parsing_logic() {
    let config_values = vec![
        "chain=evm:31337",
        "service_handler=0x1234567890123456789012345678901234567890",
        "another_key=another_value",
    ];

    // Test config format validation
    for config in &config_values {
        validate_config_format(config);
    }

    // Test aggregator config parsing
    let agg_config = config_values
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    assert_eq!(agg_config.len(), 3);
    assert!(agg_config.contains(&"chain=evm:31337".to_string()));

    // Test empty config case
    let empty_config: Option<Vec<String>> = None;
    assert!(empty_config.is_none());
}

#[test]
fn test_exec_component_config_parsing() {
    let args = vec![
        "test",
        "exec",
        "--component",
        "test.wasm",
        "--input",
        "test_input",
        "--config",
        "key1=value1",
        "--config",
        "key2=value2",
        "--config",
        "numeric_key=42",
    ];

    let config = parse_command_config(args).unwrap();
    assert_eq!(config.len(), 3);
    assert!(config.contains(&"key1=value1".to_string()));
    assert!(config.contains(&"key2=value2".to_string()));
    assert!(config.contains(&"numeric_key=42".to_string()));
}

#[test]
fn test_exec_component_config_formats() {
    // Test different value formats that should be valid
    let test_cases = vec![
        ("simple=value", true),
        ("key=123", true),
        ("boolean_key=true", true),
        ("hex_value=0x123abc", true),
        ("complex_key=value_with_underscores", true),
        ("url=https://example.com:8080/path", true),
        ("empty_value=", true),
        ("equals_in_value=key=nested=value", true), // Only first = should be delimiter
    ];

    for (config_str, should_parse) in test_cases {
        let args = vec![
            "test",
            "exec",
            "--component",
            "test.wasm",
            "--input",
            "test",
            "--config",
            config_str,
        ];

        let result = Command::try_parse_from(args);
        if should_parse {
            assert!(
                result.is_ok(),
                "Failed to parse valid config: {}",
                config_str
            );
            if let Ok(Command::Exec { config, .. }) = result {
                assert_eq!(config.len(), 1);
                assert_eq!(config[0], config_str);
            }
        } else {
            assert!(
                result.is_err(),
                "Incorrectly parsed invalid config: {}",
                config_str
            );
        }
    }
}

#[test]
fn test_service_component_config_parsing() {
    let args = vec![
        "test",
        "service",
        "workflow",
        "component",
        "--id",
        "test-workflow-123",
        "config",
        "--values",
        "database_url=postgres://localhost:5432/db",
        "--values",
        "api_key=secret123",
        "--values",
        "timeout=30",
    ];

    let parsed = Command::try_parse_from(args).unwrap();
    if let Command::Service {
        command:
            ServiceCommand::Workflow {
                command:
                    WorkflowCommand::Component {
                        command: ComponentCommand::Config { values },
                        ..
                    },
            },
        ..
    } = parsed
    {
        let config_vec = values.unwrap();
        assert_eq!(config_vec.len(), 3);
        assert!(config_vec.contains(&"database_url=postgres://localhost:5432/db".to_string()));
        assert!(config_vec.contains(&"api_key=secret123".to_string()));
        assert!(config_vec.contains(&"timeout=30".to_string()));
    } else {
        panic!("Expected Service -> Workflow -> Component -> Config command");
    }
}

#[test]
fn test_service_component_config_clear() {
    let args = vec![
        "test",
        "service",
        "workflow",
        "component",
        "--id",
        "test-workflow-123",
        "config",
    ];

    let parsed = Command::try_parse_from(args).unwrap();
    if let Command::Service {
        command:
            ServiceCommand::Workflow {
                command:
                    WorkflowCommand::Component {
                        command: ComponentCommand::Config { values },
                        ..
                    },
            },
        ..
    } = parsed
    {
        assert!(values.is_none());
    } else {
        panic!("Expected Service -> Workflow -> Component -> Config command");
    }
}

#[test]
fn test_exec_aggregator_with_shared_args() {
    // Test that shared CLI arguments work with aggregator
    let args = vec![
        "test",
        "exec-aggregator",
        "--component",
        "aggregator.wasm",
        "--config",
        "chain=evm:31337",
        "--home",
        "/custom/home",
        "--data",
        "/custom/data",
        "--log-level",
        "debug",
        "--evm-credential",
        "test-key",
        "--ipfs-gateway",
        "https://ipfs.example.com",
    ];

    let result = Command::try_parse_from(args);
    match result {
        Ok(Command::ExecAggregator {
            component,
            config,
            args,
            ..
        }) => {
            assert_eq!(component, "aggregator.wasm");
            assert!(config.is_some());
            assert_eq!(args.home, Some("/custom/home".into()));
            assert_eq!(args.data, Some("/custom/data".into()));
            assert_eq!(args.log_level, vec!["debug".to_string()]);
            assert_eq!(
                args.evm_credential,
                Some(Credential::new("test-key".to_string()))
            );
            assert_eq!(
                args.ipfs_gateway,
                Some("https://ipfs.example.com".to_string())
            );
        }
        Ok(_) => panic!("Expected ExecAggregator command"),
        Err(e) => panic!("Failed to parse ExecAggregator command: {}", e),
    }
}

#[test]
fn test_complex_json_config_values() {
    // Test that complex JSON structures can be passed without comma delimiter issues
    let test_cases = vec![
        (
            vec![
                "test",
                "exec",
                "--component",
                "test.wasm",
                "--input",
                "test",
                "--config",
                r#"resolver_config={"coin_market_cap_id":1,"threshold":1.0}"#,
            ],
            r#"resolver_config={"coin_market_cap_id":1,"threshold":1.0}"#,
        ),
        (
            vec![
                "test",
                "exec",
                "--component",
                "test.wasm",
                "--input",
                "test",
                "--config",
                r#"chains=["evm:local-1","evm:local-2","evm:mainnet"]"#,
            ],
            r#"chains=["evm:local-1","evm:local-2","evm:mainnet"]"#,
        ),
        (
            vec![
                "test",
                "exec-aggregator",
                "--component",
                "aggregator.wasm",
                "--config",
                r#"service_handlers=[{"chain":"local","address":"0x1111"},{"chain":"mainnet","address":"0x2222"}]"#,
            ],
            r#"service_handlers=[{"chain":"local","address":"0x1111"},{"chain":"mainnet","address":"0x2222"}]"#,
        ),
    ];

    for (args, expected_config) in test_cases {
        let result = Command::try_parse_from(args.clone());
        assert!(
            result.is_ok(),
            "Failed to parse command with complex JSON config: {:?}",
            args
        );

        match result.unwrap() {
            Command::Exec { config, .. } => {
                assert_eq!(config.len(), 1);
                assert_eq!(config[0], expected_config);
            }
            Command::ExecAggregator { config, .. } => {
                let config = config.unwrap();
                assert_eq!(config.len(), 1);
                assert_eq!(config[0], expected_config);
            }
            _ => panic!("Unexpected command type"),
        }
    }
}

#[test]
fn test_multiple_complex_configs() {
    // Test multiple complex configs can be passed together
    let args = vec![
        "test",
        "exec",
        "--component",
        "test.wasm",
        "--input",
        "test",
        "--config",
        r#"resolver_config={"coin_market_cap_id":1,"threshold":1.0}"#,
        "--config",
        r#"chains=["evm:local-1","evm:local-2"]"#,
        "--config",
        "simple_key=simple_value",
        "--config",
        r#"nested_object={"level1":{"level2":{"value":42}}}"#,
    ];

    let config = parse_command_config(args).unwrap();
    assert_eq!(config.len(), 4);
    assert!(
        config.contains(&r#"resolver_config={"coin_market_cap_id":1,"threshold":1.0}"#.to_string())
    );
    assert!(config.contains(&r#"chains=["evm:local-1","evm:local-2"]"#.to_string()));
    assert!(config.contains(&"simple_key=simple_value".to_string()));
    assert!(config.contains(&r#"nested_object={"level1":{"level2":{"value":42}}}"#.to_string()));
}

#[test]
fn test_config_formats() {
    // Test general config formats
    let general_configs = vec![
        "key=value",
        "number=123",
        "boolean=true",
        "url=https://example.com",
        "path=/home/user/file.txt",
        "complex=value_with_special-chars.123",
        "empty=",
        "multiple=equals=in=value",
    ];

    // Test aggregator-specific config formats
    let aggregator_configs = vec![
        ("chain", "evm:ethereum"),
        ("chain", "evm:31337"),
        (
            "service_handler",
            "0x1234567890123456789012345678901234567890",
        ),
        ("url", "https://aggregator.example.com:8080"),
        ("timeout", "30"),
        ("batch_size", "100"),
    ];

    // Validate all general configs
    for config in general_configs {
        validate_config_format(config);
    }

    // Validate aggregator configs with specific rules
    for (key, value) in aggregator_configs {
        let config_str = format!("{}={}", key, value);
        validate_config_format(&config_str);

        // Additional specific validations
        match key {
            "chain" => assert!(!value.is_empty(), "Chain name should not be empty"),
            "service_handler" => assert!(value.starts_with("0x") || !value.is_empty()),
            "url" => assert!(value.starts_with("http"), "URL should be valid"),
            _ => {} // Other keys already validated by validate_config_format
        }
    }
}
