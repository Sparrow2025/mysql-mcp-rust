// Feature: mysql-mcp-multi-datasource, Property 4: Query parameter validation
// Validates: Requirements 2.3

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::QueryTool;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Strategy to generate arbitrary strings (including empty ones)
fn arbitrary_string() -> impl Strategy<Value = String> {
    prop::option::of("[a-zA-Z0-9_-]{0,30}").prop_map(|opt| opt.unwrap_or_default())
}

// Strategy to generate arbitrary non-empty strings
fn arbitrary_non_empty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,30}"
}

// Strategy to generate whitespace-only strings
fn arbitrary_whitespace_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("[ \t\n\r]{1,10}").unwrap()
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

// Strategy to generate query parameters (datasource_key, database, query)
// This includes valid, empty, and whitespace-only strings
fn arbitrary_query_params() -> impl Strategy<Value = (String, String, String)> {
    (
        arbitrary_string(),
        arbitrary_string(),
        arbitrary_string(),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 4: Query parameter validation
    /// For any query request, if either the data source key or database name is missing,
    /// the request should be rejected
    #[test]
    fn test_query_parameter_validation(
        config in valid_datasource_config(),
        (datasource_key, database, query) in arbitrary_query_params()
    ) {
        // Create a tokio runtime for the async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            // Create a manager with one valid data source
            let valid_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager.clone(), pool_managers);

            // Execute the query with the generated parameters
            let result = tool.execute(&datasource_key, &database, &query).await;

            // Determine if the parameters are valid
            let datasource_key_valid = !datasource_key.is_empty() && datasource_key == valid_key;
            let database_valid = !database.trim().is_empty();
            let query_valid = !query.trim().is_empty();

            // If any required parameter is missing or invalid, the request should be rejected
            if datasource_key.is_empty() || database.is_empty() || query.trim().is_empty() {
                // Should fail with InvalidStatement error
                prop_assert!(result.is_err(),
                    "Expected query to fail with empty parameters: datasource_key='{}', database='{}', query='{}'",
                    datasource_key, database, query);
                
                if let Err(e) = result {
                    // Should be InvalidStatement error for empty parameters
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Expected InvalidStatement error for empty parameters, but got: {:?}",
                        e
                    );
                }
            } else if datasource_key != valid_key {
                // Should fail with InvalidDataSourceKey error
                prop_assert!(result.is_err(),
                    "Expected query to fail with invalid datasource key: '{}'",
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
            // a real database connection. This test focuses on parameter validation only.
            
            Ok(())
        });
    }

    /// Property 4a: Empty datasource key is always rejected
    /// For any query with an empty datasource key, the request should be rejected
    #[test]
    fn test_empty_datasource_key_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        query in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            let result = tool.execute("", &database, &query).await;
            
            prop_assert!(result.is_err(),
                "Expected query to fail with empty datasource key");
            
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

    /// Property 4b: Empty database name is always rejected
    /// For any query with an empty database name, the request should be rejected
    #[test]
    fn test_empty_database_rejected(
        config in valid_datasource_config(),
        query in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, "", &query).await;
            
            prop_assert!(result.is_err(),
                "Expected query to fail with empty database name");
            
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

    /// Property 4c: Empty query is always rejected
    /// For any query with an empty query string, the request should be rejected
    #[test]
    fn test_empty_query_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, "").await;
            
            prop_assert!(result.is_err(),
                "Expected query to fail with empty query string");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Expected InvalidStatement error for empty query, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 4d: Whitespace-only query is always rejected
    /// For any query with a whitespace-only query string, the request should be rejected
    #[test]
    fn test_whitespace_query_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        whitespace_query in arbitrary_whitespace_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, &whitespace_query).await;
            
            prop_assert!(result.is_err(),
                "Expected query to fail with whitespace-only query string: '{}'",
                whitespace_query);
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Expected InvalidStatement error for whitespace query, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 4e: Invalid datasource key is always rejected
    /// For any query with a datasource key that doesn't exist, the request should be rejected
    #[test]
    fn test_invalid_datasource_key_rejected(
        config in valid_datasource_config(),
        invalid_key in arbitrary_non_empty_string(),
        database in arbitrary_non_empty_string(),
        query in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let valid_key = config.key.clone();
            
            // Only test if the invalid key is actually different from the valid key
            if invalid_key != valid_key {
                let configs = vec![config];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = QueryTool::new(manager, pool_managers);

                let result = tool.execute(&invalid_key, &database, &query).await;
                
                prop_assert!(result.is_err(),
                    "Expected query to fail with invalid datasource key: '{}'",
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

    /// Property 4f: All three parameters required together
    /// For any query, all three parameters (datasource_key, database, query) must be
    /// non-empty for the validation to pass (though execution may still fail for other reasons)
    #[test]
    fn test_all_parameters_required(
        config in valid_datasource_config(),
        (datasource_key, database, query) in arbitrary_query_params()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let valid_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, &query).await;

            // Count how many parameters are invalid
            let empty_datasource = datasource_key.is_empty();
            let empty_database = database.is_empty();
            let empty_query = query.trim().is_empty();
            let invalid_key = !datasource_key.is_empty() && datasource_key != valid_key;

            // If any parameter is empty or key is invalid, should fail
            if empty_datasource || empty_database || empty_query || invalid_key {
                prop_assert!(result.is_err(),
                    "Expected query to fail when parameters are invalid: datasource_key='{}' (empty={}, invalid={}), database='{}' (empty={}), query='{}' (empty={})",
                    datasource_key, empty_datasource, invalid_key, database, empty_database, query, empty_query);
            }
            // Note: We don't assert success here because that would require a real database
            
            Ok(())
        });
    }
}
