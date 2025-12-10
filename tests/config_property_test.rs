// Feature: mysql-mcp-multi-datasource, Property 1: Configuration validation completeness
// Validates: Requirements 1.2

// Feature: mysql-mcp-multi-datasource, Property 2: Invalid configuration handling
// Validates: Requirements 1.3

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig, ServerConfig};
use proptest::prelude::*;

// Strategy to generate arbitrary strings (including empty ones)
fn arbitrary_string() -> impl Strategy<Value = String> {
    prop::option::of("[a-zA-Z0-9_-]{0,20}").prop_map(|opt| opt.unwrap_or_default())
}

// Strategy to generate arbitrary non-empty strings
fn arbitrary_non_empty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,20}"
}

// Strategy to generate arbitrary ports (including 0)
fn arbitrary_port() -> impl Strategy<Value = u16> {
    0u16..=65535u16
}

// Strategy to generate arbitrary valid ports (excluding 0)
fn arbitrary_valid_port() -> impl Strategy<Value = u16> {
    1u16..=65535u16
}

// Strategy to generate arbitrary PoolConfig
fn arbitrary_pool_config() -> impl Strategy<Value = PoolConfig> {
    (1u32..=100u32, 0u32..=50u32, 1u64..=300u64, 1u64..=600u64, 1u64..=3600u64)
        .prop_map(|(max_conn, min_conn, conn_timeout, idle_timeout, max_lifetime)| {
            PoolConfig {
                max_connections: max_conn,
                min_connections: min_conn.min(max_conn), // Ensure min <= max
                connection_timeout_secs: conn_timeout,
                idle_timeout_secs: idle_timeout,
                max_lifetime_secs: max_lifetime,
            }
        })
}

// Strategy to generate a DataSourceConfig with potentially missing required fields
fn arbitrary_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    (
        arbitrary_string(),
        arbitrary_non_empty_string(),
        arbitrary_string(),
        arbitrary_port(),
        arbitrary_string(),
        arbitrary_string(),
        prop::collection::vec(arbitrary_non_empty_string(), 0..5),
        arbitrary_pool_config(),
    )
        .prop_map(
            |(key, name, host, port, username, password, databases, pool_config)| {
                DataSourceConfig {
                    key,
                    name,
                    host,
                    port,
                    username,
                    password,
                    databases,
                    pool_config,
                    permission: mysql_mcp_server::config::Permission::default(),
                }
            },
        )
}

// Strategy to generate a valid DataSourceConfig
fn valid_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    (
        arbitrary_non_empty_string(),
        arbitrary_non_empty_string(),
        arbitrary_non_empty_string(),
        arbitrary_valid_port(),
        arbitrary_non_empty_string(),
        arbitrary_non_empty_string(),
        prop::collection::vec(arbitrary_non_empty_string(), 0..5),
        arbitrary_pool_config(),
    )
        .prop_map(
            |(key, name, host, port, username, password, databases, pool_config)| {
                DataSourceConfig {
                    key,
                    name,
                    host,
                    port,
                    username,
                    password,
                    databases,
                    pool_config,
                    permission: mysql_mcp_server::config::Permission::default(),
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 1: Configuration validation completeness
    /// For any data source configuration, if any required field (host, port, username, password) 
    /// is missing or invalid, the validation should reject the configuration
    #[test]
    fn test_config_validation_rejects_missing_required_fields(
        ds in arbitrary_datasource_config()
    ) {
        let result = ds.validate();
        
        // Check if any required field is missing or invalid
        let has_missing_or_invalid_field = 
            ds.key.is_empty() ||
            ds.host.is_empty() ||
            ds.port == 0 ||
            ds.username.is_empty() ||
            ds.password.is_empty() ||
            ds.pool_config.max_connections == 0 ||
            ds.pool_config.min_connections > ds.pool_config.max_connections;
        
        if has_missing_or_invalid_field {
            // If any required field is missing or invalid, validation should fail
            prop_assert!(result.is_err(), 
                "Expected validation to fail for config with missing/invalid fields: key='{}', host='{}', port={}, username='{}', password='{}', max_conn={}, min_conn={}",
                ds.key, ds.host, ds.port, ds.username, ds.password, ds.pool_config.max_connections, ds.pool_config.min_connections
            );
        } else {
            // If all required fields are present and valid, validation should succeed
            prop_assert!(result.is_ok(),
                "Expected validation to succeed for config with all required fields: {:?}",
                result
            );
        }
    }

    /// Property 1b: Valid configurations always pass validation
    /// For any data source configuration with all required fields present and valid,
    /// the validation should succeed
    #[test]
    fn test_valid_config_always_passes_validation(
        ds in valid_datasource_config()
    ) {
        let result = ds.validate();
        prop_assert!(result.is_ok(),
            "Expected validation to succeed for valid config, but got error: {:?}",
            result
        );
    }

    /// Property 1c: Missing key is always rejected
    /// For any data source configuration with an empty key, validation should fail
    #[test]
    fn test_missing_key_rejected(
        name in arbitrary_non_empty_string(),
        host in arbitrary_non_empty_string(),
        port in arbitrary_valid_port(),
        username in arbitrary_non_empty_string(),
        password in arbitrary_non_empty_string(),
    ) {
        let ds = DataSourceConfig {
            key: "".to_string(),
            name,
            host,
            port,
            username,
            password,
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        };
        
        let result = ds.validate();
        prop_assert!(result.is_err(), "Expected validation to fail for empty key");
    }

    /// Property 1d: Missing host is always rejected
    /// For any data source configuration with an empty host, validation should fail
    #[test]
    fn test_missing_host_rejected(
        key in arbitrary_non_empty_string(),
        port in arbitrary_valid_port(),
        username in arbitrary_non_empty_string(),
        password in arbitrary_non_empty_string(),
    ) {
        let ds = DataSourceConfig {
            key,
            name: "Test".to_string(),
            host: "".to_string(),
            port,
            username,
            password,
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        };
        
        let result = ds.validate();
        prop_assert!(result.is_err(), "Expected validation to fail for empty host");
    }

    /// Property 1e: Invalid port (0) is always rejected
    /// For any data source configuration with port 0, validation should fail
    #[test]
    fn test_invalid_port_rejected(
        key in arbitrary_non_empty_string(),
        host in arbitrary_non_empty_string(),
        username in arbitrary_non_empty_string(),
        password in arbitrary_non_empty_string(),
    ) {
        let ds = DataSourceConfig {
            key,
            name: "Test".to_string(),
            host,
            port: 0,
            username,
            password,
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        };
        
        let result = ds.validate();
        prop_assert!(result.is_err(), "Expected validation to fail for port 0");
    }

    /// Property 1f: Missing username is always rejected
    /// For any data source configuration with an empty username, validation should fail
    #[test]
    fn test_missing_username_rejected(
        key in arbitrary_non_empty_string(),
        host in arbitrary_non_empty_string(),
        port in arbitrary_valid_port(),
        password in arbitrary_non_empty_string(),
    ) {
        let ds = DataSourceConfig {
            key,
            name: "Test".to_string(),
            host,
            port,
            username: "".to_string(),
            password,
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        };
        
        let result = ds.validate();
        prop_assert!(result.is_err(), "Expected validation to fail for empty username");
    }

    /// Property 1g: Missing password is always rejected
    /// For any data source configuration with an empty password, validation should fail
    #[test]
    fn test_missing_password_rejected(
        key in arbitrary_non_empty_string(),
        host in arbitrary_non_empty_string(),
        port in arbitrary_valid_port(),
        username in arbitrary_non_empty_string(),
    ) {
        let ds = DataSourceConfig {
            key,
            name: "Test".to_string(),
            host,
            port,
            username,
            password: "".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        };
        
        let result = ds.validate();
        prop_assert!(result.is_err(), "Expected validation to fail for empty password");
    }
}


// ============================================================================
// Property 2: Invalid configuration handling
// ============================================================================

// Strategy to generate a mix of valid and invalid data source configs
fn mixed_datasource_configs() -> impl Strategy<Value = Vec<DataSourceConfig>> {
    prop::collection::vec(
        prop::bool::ANY.prop_flat_map(|is_valid| {
            if is_valid {
                valid_datasource_config().boxed()
            } else {
                arbitrary_datasource_config().boxed()
            }
        }),
        1..10, // Generate 1 to 10 data sources
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 2: Invalid configuration handling
    /// For any data source configuration with invalid values, the server should log an error 
    /// and skip that data source without crashing
    #[test]
    fn test_server_filters_invalid_datasources_without_crashing(
        data_sources in mixed_datasource_configs()
    ) {
        // Count how many are valid before filtering
        let valid_count_before = data_sources.iter()
            .filter(|ds| ds.validate().is_ok())
            .count();
        
        let mut config = ServerConfig {
            data_sources,
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        // This should not panic/crash
        config.validate_and_filter();
        
        // After filtering, all remaining data sources should be valid
        for ds in &config.data_sources {
            prop_assert!(ds.validate().is_ok(), 
                "After filtering, found invalid data source: {:?}", ds);
        }
        
        // The number of remaining data sources should equal the number of valid ones before
        prop_assert_eq!(config.data_sources.len(), valid_count_before,
            "Expected {} valid data sources after filtering, but got {}",
            valid_count_before, config.data_sources.len());
    }

    /// Property 2b: Server with all invalid configs results in empty list
    /// For any server config where all data sources are invalid, filtering should result 
    /// in an empty data source list (without crashing)
    #[test]
    fn test_server_handles_all_invalid_configs(
        invalid_configs in prop::collection::vec(arbitrary_datasource_config(), 1..5)
    ) {
        // Ensure all configs are actually invalid
        let all_invalid = invalid_configs.iter().all(|ds| ds.validate().is_err());
        
        if all_invalid {
            let mut config = ServerConfig {
                data_sources: invalid_configs,
                query_timeout_secs: 30,
                stream_chunk_size: 1000,
            };
            
            // This should not panic/crash
            config.validate_and_filter();
            
            // After filtering, should have no data sources
            prop_assert_eq!(config.data_sources.len(), 0,
                "Expected 0 data sources after filtering all invalid configs");
        }
    }

    /// Property 2c: Server with all valid configs keeps all
    /// For any server config where all data sources are valid, filtering should keep all of them
    #[test]
    fn test_server_keeps_all_valid_configs(
        valid_configs in prop::collection::vec(valid_datasource_config(), 1..5)
    ) {
        let original_count = valid_configs.len();
        
        let mut config = ServerConfig {
            data_sources: valid_configs,
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        // This should not panic/crash
        config.validate_and_filter();
        
        // After filtering, should still have all data sources
        prop_assert_eq!(config.data_sources.len(), original_count,
            "Expected all {} valid data sources to remain after filtering", original_count);
        
        // All should still be valid
        for ds in &config.data_sources {
            prop_assert!(ds.validate().is_ok(), 
                "Found invalid data source after filtering: {:?}", ds);
        }
    }

    /// Property 2d: Filtering is idempotent
    /// For any server config, filtering twice should produce the same result as filtering once
    #[test]
    fn test_filtering_is_idempotent(
        data_sources in mixed_datasource_configs()
    ) {
        let mut config1 = ServerConfig {
            data_sources: data_sources.clone(),
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        let mut config2 = ServerConfig {
            data_sources,
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        // Filter once
        config1.validate_and_filter();
        let count_after_first = config1.data_sources.len();
        
        // Filter again
        config1.validate_and_filter();
        let count_after_second = config1.data_sources.len();
        
        // Filter the second config twice
        config2.validate_and_filter();
        config2.validate_and_filter();
        
        prop_assert_eq!(count_after_first, count_after_second,
            "Filtering should be idempotent");
        
        prop_assert_eq!(config1.data_sources.len(), config2.data_sources.len(),
            "Both configs should have same number of data sources after filtering");
    }
}
