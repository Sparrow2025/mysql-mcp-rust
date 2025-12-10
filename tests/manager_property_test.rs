// Feature: mysql-mcp-multi-datasource, Property 18: Data source key uniqueness
// Validates: Requirements 8.2
//
// Feature: mysql-mcp-multi-datasource, Property 20: Key-to-credentials mapping correctness
// Validates: Requirements 8.5

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::manager::DataSourceManager;
use proptest::prelude::*;

// Strategy to generate arbitrary non-empty strings for keys
fn arbitrary_key() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,20}"
}

// Strategy to generate arbitrary non-empty strings
fn arbitrary_non_empty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,20}"
}

// Strategy to generate arbitrary valid ports
fn arbitrary_valid_port() -> impl Strategy<Value = u16> {
    1u16..=65535u16
}

// Strategy to generate a valid DataSourceConfig with random values
fn valid_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    (
        arbitrary_key(),
        arbitrary_non_empty_string(),
        arbitrary_non_empty_string(),
        arbitrary_valid_port(),
        arbitrary_non_empty_string(),
        arbitrary_non_empty_string(),
    )
        .prop_map(|(key, name, host, port, username, password)| DataSourceConfig {
            key,
            name,
            host,
            port,
            username,
            password,
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        })
}

// Strategy to generate a vector of data source configs with at least one duplicate key
fn configs_with_duplicate_keys() -> impl Strategy<Value = Vec<DataSourceConfig>> {
    (
        valid_datasource_config(),
        prop::collection::vec(valid_datasource_config(), 1..5),
        0usize..5usize, // Index where to insert the duplicate
    )
        .prop_map(|(base_config, mut other_configs, insert_idx)| {
            // Create a duplicate of the base config with the same key
            let duplicate = DataSourceConfig {
                key: base_config.key.clone(),
                name: format!("{}-duplicate", base_config.name),
                host: base_config.host.clone(),
                port: base_config.port,
                username: base_config.username.clone(),
                password: base_config.password.clone(),
                databases: vec![],
                pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
            };

            // Insert the duplicate at a random position
            let insert_pos = insert_idx.min(other_configs.len());
            other_configs.insert(insert_pos, duplicate);
            other_configs.insert(0, base_config);

            other_configs
        })
}

// Strategy to generate a vector of data source configs with unique keys
fn configs_with_unique_keys() -> impl Strategy<Value = Vec<DataSourceConfig>> {
    prop::collection::vec(valid_datasource_config(), 1..10)
        .prop_map(|mut configs| {
            // Ensure all keys are unique by appending index
            for (i, config) in configs.iter_mut().enumerate() {
                config.key = format!("{}-{}", config.key, i);
            }
            configs
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 18: Data source key uniqueness
    /// For any set of configured data sources, all generated or assigned keys should be unique.
    /// When duplicate keys are present, the DataSourceManager should reject the configuration.
    #[test]
    fn test_datasource_manager_rejects_duplicate_keys(
        configs in configs_with_duplicate_keys()
    ) {
        // Verify that there are indeed duplicate keys in the input
        let mut keys = std::collections::HashSet::new();
        let has_duplicates = !configs.iter().all(|c| keys.insert(&c.key));
        
        prop_assert!(has_duplicates, "Test setup error: expected duplicate keys in input");

        // Create a tokio runtime for the async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        // Attempt to create a DataSourceManager with duplicate keys
        let result = rt.block_on(DataSourceManager::new(configs));
        
        // The manager should reject the configuration with duplicate keys
        prop_assert!(result.is_err(), 
            "Expected DataSourceManager::new to fail with duplicate keys, but it succeeded");
        
        // Verify the error message mentions duplicate keys
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            prop_assert!(error_msg.to_lowercase().contains("duplicate"), 
                "Expected error message to mention 'duplicate', but got: {}", error_msg);
        }
    }

    /// Property 18b: DataSourceManager accepts unique keys
    /// For any set of data source configurations with unique keys,
    /// the DataSourceManager should successfully create and register all data sources.
    #[test]
    fn test_datasource_manager_accepts_unique_keys(
        configs in configs_with_unique_keys()
    ) {
        // Verify that all keys are unique
        let mut keys = std::collections::HashSet::new();
        let all_unique = configs.iter().all(|c| keys.insert(&c.key));
        
        prop_assert!(all_unique, "Test setup error: expected all unique keys in input");

        let expected_count = configs.len();

        // Create a tokio runtime for the async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        // Attempt to create a DataSourceManager with unique keys
        let result = rt.block_on(DataSourceManager::new(configs.clone()));
        
        // The manager should accept the configuration with unique keys
        prop_assert!(result.is_ok(), 
            "Expected DataSourceManager::new to succeed with unique keys, but got error: {:?}", 
            result.err());
        
        if let Ok(manager) = result {
            // Verify that all data sources were registered
            prop_assert_eq!(manager.count(), expected_count,
                "Expected {} data sources to be registered, but got {}",
                expected_count, manager.count());
            
            // Verify that all keys can be retrieved
            for config in &configs {
                prop_assert!(manager.get_source(&config.key).is_some(),
                    "Expected to find data source with key '{}', but it was not found",
                    config.key);
            }
        }
    }

    /// Property 20: Key-to-credentials mapping correctness
    /// For any valid data source key, the server should correctly map it to
    /// the corresponding data source credentials internally.
    #[test]
    fn test_key_to_credentials_mapping_correctness(
        configs in configs_with_unique_keys()
    ) {
        // Verify that all keys are unique
        let mut keys = std::collections::HashSet::new();
        let all_unique = configs.iter().all(|c| keys.insert(&c.key));
        
        prop_assert!(all_unique, "Test setup error: expected all unique keys in input");

        // Create a tokio runtime for the async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        // Create a DataSourceManager with the configs
        let result = rt.block_on(DataSourceManager::new(configs.clone()));
        
        prop_assert!(result.is_ok(), 
            "Expected DataSourceManager::new to succeed, but got error: {:?}", 
            result.err());
        
        if let Ok(manager) = result {
            // For each config, verify that the key maps to the correct credentials
            for config in &configs {
                // Get the data source by key
                let retrieved = manager.get_source(&config.key);
                
                prop_assert!(retrieved.is_some(),
                    "Expected to find data source with key '{}', but it was not found",
                    config.key);
                
                if let Some(retrieved_config) = retrieved {
                    // Verify that all fields match the original configuration
                    prop_assert_eq!(&retrieved_config.key, &config.key,
                        "Key mismatch for data source '{}'", config.key);
                    
                    prop_assert_eq!(&retrieved_config.name, &config.name,
                        "Name mismatch for data source '{}'", config.key);
                    
                    prop_assert_eq!(&retrieved_config.host, &config.host,
                        "Host mismatch for data source '{}'", config.key);
                    
                    prop_assert_eq!(retrieved_config.port, config.port,
                        "Port mismatch for data source '{}'", config.key);
                    
                    prop_assert_eq!(&retrieved_config.username, &config.username,
                        "Username mismatch for data source '{}'", config.key);
                    
                    // Most importantly, verify that the password (credential) is correctly mapped
                    prop_assert_eq!(&retrieved_config.password, &config.password,
                        "Password (credential) mismatch for data source '{}'", config.key);
                    
                    prop_assert_eq!(&retrieved_config.databases, &config.databases,
                        "Databases list mismatch for data source '{}'", config.key);
                }
            }
            
            // Also verify that invalid keys don't map to any credentials
            let invalid_keys = vec!["nonexistent", "invalid-key", ""];
            for invalid_key in invalid_keys {
                let retrieved = manager.get_source(invalid_key);
                prop_assert!(retrieved.is_none(),
                    "Expected invalid key '{}' to return None, but got Some", invalid_key);
                
                // Verify that validate_key also rejects invalid keys
                let validation_result = manager.validate_key(invalid_key);
                prop_assert!(validation_result.is_err(),
                    "Expected validate_key to fail for invalid key '{}', but it succeeded", invalid_key);
            }
        }
    }
}
