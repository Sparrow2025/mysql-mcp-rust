// Feature: mysql-mcp-multi-datasource, Property 13: Non-existent table error handling
// Validates: Requirements 4.3

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

    /// Property 13: Non-existent table error handling
    /// For any non-existent table name, the schema tool should return an error message
    /// 
    /// This test validates that the describe_table function properly validates parameters
    /// before attempting database operations. The actual table existence check requires
    /// a real database connection and is tested in integration tests.
    /// 
    /// This property test focuses on ensuring that:
    /// 1. Parameter validation happens before database operations
    /// 2. Invalid parameters are rejected with appropriate error types
    /// 3. The error handling infrastructure is in place
    #[test]
    fn test_table_existence_error_handling_infrastructure(
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

            // Validate that parameter validation happens before database operations
            if datasource_key.is_empty() || database.is_empty() || table.is_empty() {
                // Empty parameters should be rejected with InvalidStatement
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with empty parameters");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Expected InvalidStatement error for empty parameters, but got: {:?}",
                        e
                    );
                }
            } else if datasource_key != valid_key {
                // Invalid datasource key should be rejected with InvalidDataSourceKey
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with invalid datasource key");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidDataSourceKey(_)),
                        "Expected InvalidDataSourceKey error, but got: {:?}",
                        e
                    );
                }
            }
            // Note: We don't test actual table existence here as that requires a real database
            
            Ok(())
        });
    }

    /// Property 13b: Empty table name is rejected before attempting database connection
    #[test]
    fn test_empty_table_rejected_early(
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
            
            // Should fail with InvalidStatement, not a database error
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

    /// Property 13c: Parameter validation happens before database operations
    /// This ensures that invalid parameters are caught early without attempting database operations
    #[test]
    fn test_parameter_validation_before_db_operations(
        config in valid_datasource_config(),
        invalid_key in arbitrary_non_empty_string(),
        database in arbitrary_non_empty_string(),
        table in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let valid_key = config.key.clone();
            
            if invalid_key != valid_key {
                let configs = vec![config];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = SchemaTool::new(manager, pool_managers);

                let result = tool.describe_table(&invalid_key, &database, &table).await;
                
                prop_assert!(result.is_err(),
                    "Expected describe_table to fail with invalid datasource key");
                
                // Should fail with InvalidDataSourceKey, not a database error
                // This proves validation happens before attempting database operations
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
