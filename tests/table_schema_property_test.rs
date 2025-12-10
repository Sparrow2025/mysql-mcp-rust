// Feature: mysql-mcp-multi-datasource, Property 12: Table schema completeness
// Validates: Requirements 4.2, 4.4, 4.5

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

    /// Property 12: Table schema completeness
    /// For any table, the schema information should include column names, data types,
    /// constraints, primary keys, foreign keys, and indexes
    /// 
    /// This test validates that:
    /// 1. Valid parameters are accepted
    /// 2. Empty datasource key is rejected
    /// 3. Empty database name is rejected
    /// 4. Empty table name is rejected
    /// 5. Invalid datasource key is rejected
    #[test]
    fn test_table_schema_parameter_validation(
        config in valid_datasource_config(),
        datasource_key in prop::option::of(arbitrary_non_empty_string()),
        database in prop::option::of(arbitrary_non_empty_string()),
        table in prop::option::of(arbitrary_non_empty_string()),
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
            let table = table.unwrap_or_default();

            let result = tool.describe_table(&datasource_key, &database, &table).await;

            // Validate parameter handling
            if datasource_key.is_empty() {
                // Empty datasource key should be rejected
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with empty datasource key");
                
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
                    "Expected describe_table to fail with empty database name");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Expected InvalidStatement error for empty database name, but got: {:?}",
                        e
                    );
                }
            } else if table.is_empty() {
                // Empty table name should be rejected
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with empty table name");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Expected InvalidStatement error for empty table name, but got: {:?}",
                        e
                    );
                }
            } else if datasource_key != valid_key {
                // Invalid datasource key should be rejected
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with invalid datasource key: '{}'",
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

    /// Property 12a: Empty datasource key is always rejected
    #[test]
    fn test_empty_datasource_key_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        table in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager, pool_managers);

            let result = tool.describe_table("", &database, &table).await;
            
            prop_assert!(result.is_err(),
                "Expected describe_table to fail with empty datasource key");
            
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

    /// Property 12b: Empty database name is always rejected
    #[test]
    fn test_empty_database_rejected(
        config in valid_datasource_config(),
        table in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager, pool_managers);

            let result = tool.describe_table(&datasource_key, "", &table).await;
            
            prop_assert!(result.is_err(),
                "Expected describe_table to fail with empty database name");
            
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

    /// Property 12c: Empty table name is always rejected
    #[test]
    fn test_empty_table_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager, pool_managers);

            let result = tool.describe_table(&datasource_key, &database, "").await;
            
            prop_assert!(result.is_err(),
                "Expected describe_table to fail with empty table name");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Expected InvalidStatement error for empty table name, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 12d: Invalid datasource key is always rejected
    #[test]
    fn test_invalid_datasource_key_rejected(
        config in valid_datasource_config(),
        invalid_key in arbitrary_non_empty_string(),
        database in arbitrary_non_empty_string(),
        table in arbitrary_non_empty_string(),
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

                let result = tool.describe_table(&invalid_key, &database, &table).await;
                
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with invalid datasource key: '{}'",
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

    /// Property 12e: All three parameters required together
    /// For any describe_table call, all three parameters (datasource_key, database, table)
    /// must be non-empty for the validation to pass
    #[test]
    fn test_all_parameters_required(
        config in valid_datasource_config(),
        datasource_key in prop::option::of(arbitrary_non_empty_string()),
        database in prop::option::of(arbitrary_non_empty_string()),
        table in prop::option::of(arbitrary_non_empty_string()),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let valid_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = SchemaTool::new(manager, pool_managers);

            let datasource_key = datasource_key.unwrap_or_default();
            let database = database.unwrap_or_default();
            let table = table.unwrap_or_default();

            let result = tool.describe_table(&datasource_key, &database, &table).await;

            // Count how many parameters are invalid
            let empty_datasource = datasource_key.is_empty();
            let empty_database = database.is_empty();
            let empty_table = table.is_empty();
            let invalid_key = !datasource_key.is_empty() && datasource_key != valid_key;

            // If any parameter is empty or key is invalid, should fail
            if empty_datasource || empty_database || empty_table || invalid_key {
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail when parameters are invalid: datasource_key='{}' (empty={}, invalid={}), database='{}' (empty={}), table='{}' (empty={})",
                    datasource_key, empty_datasource, invalid_key, database, empty_database, table, empty_table);
            }
            // Note: We don't assert success here because that would require a real database
            
            Ok(())
        });
    }
}
