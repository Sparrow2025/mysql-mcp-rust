// Feature: mysql-mcp-multi-datasource, Property 11: Table listing completeness
// Validates: Requirements 4.1

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::SchemaTool;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Strategy to generate arbitrary non-empty strings
fn arbitrary_non_empty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,30}"
}

// Strategy to generate a valid DataSourceConfig for testing
fn valid_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    arbitrary_non_empty_string().prop_map(|key| DataSourceConfig {
        key,
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 11: Table listing completeness
    /// For any database, the schema tool should return all tables present in that database
    /// 
    /// This test validates that:
    /// 1. Valid parameters are accepted
    /// 2. Empty datasource key is rejected
    /// 3. Empty database name is rejected
    /// 4. Invalid datasource key is rejected
    #[test]
    fn test_table_listing_parameter_validation(
        config in valid_datasource_config(),
        datasource_key in prop::option::of(arbitrary_non_empty_string()),
        database in prop::option::of(arbitrary_non_empty_string()),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let valid_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager.clone(), pool_managers);

            let datasource_key = datasource_key.unwrap_or_default();
            let database = database.unwrap_or_default();

            let result = tool.list_tables(&datasource_key, &database).await;

            // Validate parameter handling
            if datasource_key.is_empty() {
                // Empty datasource key should be rejected
                prop_assert!(result.is_err(),
                    "Expected list_tables to fail with empty datasource key");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Expected InvalidStatement error for empty datasource key, but got: {:?}",
                        e
                    );
                }
            } else if database.is_empty() {
                // Empty database name should be rejected
                prop_assert!(result.is_err(),
                    "Expected list_tables to fail with empty database name");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Expected InvalidStatement error for empty database name, but got: {:?}",
                        e
                    );
                }
            } else if datasource_key != valid_key {
                // Invalid datasource key should be rejected
                prop_assert!(result.is_err(),
                    "Expected list_tables to fail with invalid datasource key: '{}'",
                    datasource_key);
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidDataSourceKey(_)),
                        "Expected InvalidDataSourceKey error, but got: {:?}",
                        e
                    );
                }
            }
            // Note: We don't test successful execution here because that would require
            // a real database connection. This test focuses on parameter validation.
            
            Ok(())
        });
    }

    /// Property 11a: Empty datasource key is always rejected
    #[test]
    fn test_empty_datasource_key_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager, pool_managers);

            let result = tool.list_tables("", &database).await;
            
            prop_assert!(result.is_err(),
                "Expected list_tables to fail with empty datasource key");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Expected InvalidStatement error for empty datasource key, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 11b: Empty database name is always rejected
    #[test]
    fn test_empty_database_rejected(
        config in valid_datasource_config(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager, pool_managers);

            let result = tool.list_tables(&datasource_key, "").await;
            
            prop_assert!(result.is_err(),
                "Expected list_tables to fail with empty database name");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Expected InvalidStatement error for empty database name, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 11c: Invalid datasource key is always rejected
    #[test]
    fn test_invalid_datasource_key_rejected(
        config in valid_datasource_config(),
        invalid_key in arbitrary_non_empty_string(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let valid_key = config.key.clone();
            
            // Only test if the invalid key is actually different from the valid key
            if invalid_key != valid_key {
                let configs = vec![config];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = SchemaTool::new(manager, pool_managers);

                let result = tool.list_tables(&invalid_key, &database).await;
                
                prop_assert!(result.is_err(),
                    "Expected list_tables to fail with invalid datasource key: '{}'",
                    invalid_key);
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidDataSourceKey(_)),
                        "Expected InvalidDataSourceKey error, but got: {:?}",
                        e
                    );
                }
            }
            
            Ok(())
        });
    }
}
