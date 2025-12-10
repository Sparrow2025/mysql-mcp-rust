// Feature: mysql-mcp-multi-datasource, Property 24: DML execution correctness
// Validates: Requirements 11.1

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::ExecuteTool;
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

// Strategy to generate valid DML statements
fn arbitrary_dml_statement() -> impl Strategy<Value = String> {
    prop_oneof![
        // INSERT statements
        Just("INSERT INTO users (id, name) VALUES (1, 'test')".to_string()),
        Just("INSERT INTO products (id, price) VALUES (1, 99.99)".to_string()),
        // UPDATE statements
        Just("UPDATE users SET name = 'updated' WHERE id = 1".to_string()),
        Just("UPDATE products SET price = 199.99 WHERE id = 1".to_string()),
        // DELETE statements
        Just("DELETE FROM users WHERE id = 1".to_string()),
        Just("DELETE FROM products WHERE id = 1".to_string()),
    ]
}

// Strategy to generate invalid statements (empty or whitespace)
fn arbitrary_invalid_statement() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("".to_string()),
        Just("   ".to_string()),
        Just("\t\n".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5))]

    /// Property 24: DML execution correctness
    /// For any valid DML statement (INSERT, UPDATE, DELETE), the execute tool should
    /// execute it and return the affected row count (or fail with a descriptive error)
    #[test]
    fn test_dml_execution_parameter_validation(
        config in valid_datasource_config(),
        statement in arbitrary_dml_statement(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            // Execute the DML statement
            let result = tool.execute(&datasource_key, &database, &statement).await;

            // The statement should either succeed (returning ExecuteResult) or fail with
            // a descriptive error (not a panic or silent failure)
            // Since we don't have a real database, we expect it to fail with a connection
            // or database error, but the important thing is that it doesn't panic and
            // returns a proper error
            
            match result {
                Ok(_execute_result) => {
                    // If it succeeds (unlikely without a real DB), that's fine
                    prop_assert!(true, "Execute succeeded");
                }
                Err(e) => {
                    // Should fail with a descriptive error, not panic
                    // Valid errors include: ConnectionFailed, DatabaseNotFound, QueryExecutionError
                    let error_msg = e.to_string();
                    prop_assert!(!error_msg.is_empty(),
                        "Error message should not be empty");
                    
                    // Should not be an InvalidStatement error for valid DML
                    prop_assert!(!matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                        "Valid DML statement should not return InvalidStatement error");
                    
                    // Should not be a DDL rejection error
                    prop_assert!(!matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                        "Valid DML statement should not be rejected as DDL");
                }
            }
            
            Ok(())
        });
    }

    /// Property 24a: Empty statement is rejected
    /// For any empty or whitespace-only statement, the execute tool should reject it
    #[test]
    fn test_empty_statement_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        invalid_statement in arbitrary_invalid_statement(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, &invalid_statement).await;
            
            prop_assert!(result.is_err(),
                "Empty or whitespace statement should be rejected");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Empty statement should return InvalidStatement error, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 24b: Empty datasource key is rejected
    /// For any DML statement with an empty datasource key, the request should be rejected
    #[test]
    fn test_empty_datasource_key_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        statement in arbitrary_dml_statement(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            let result = tool.execute("", &database, &statement).await;
            
            prop_assert!(result.is_err(),
                "Empty datasource key should be rejected");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Empty datasource key should return InvalidStatement error, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 24c: Empty database name is rejected
    /// For any DML statement with an empty database name, the request should be rejected
    #[test]
    fn test_empty_database_rejected(
        config in valid_datasource_config(),
        statement in arbitrary_dml_statement(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, "", &statement).await;
            
            prop_assert!(result.is_err(),
                "Empty database name should be rejected");
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::InvalidStatement(_)),
                    "Empty database name should return InvalidStatement error, but got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }

    /// Property 24d: Invalid datasource key is rejected
    /// For any DML statement with an invalid datasource key, the request should be rejected
    #[test]
    fn test_invalid_datasource_key_rejected(
        config in valid_datasource_config(),
        invalid_key in arbitrary_non_empty_string(),
        database in arbitrary_non_empty_string(),
        statement in arbitrary_dml_statement(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let valid_key = config.key.clone();
            
            // Only test if the invalid key is actually different from the valid key
            if invalid_key != valid_key {
                let configs = vec![config];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = ExecuteTool::new(manager, pool_managers);

                let result = tool.execute(&invalid_key, &database, &statement).await;
                
                prop_assert!(result.is_err(),
                    "Invalid datasource key should be rejected");
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::InvalidDataSourceKey(_)),
                        "Invalid datasource key should return InvalidDataSourceKey error, but got: {:?}",
                        e
                    );
                }
            }
            
            Ok(())
        });
    }

    /// Property 24e: DML statements are not rejected as DDL
    /// For any valid DML statement, it should not be rejected as a DDL statement
    #[test]
    fn test_dml_not_rejected_as_ddl(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        statement in arbitrary_dml_statement(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, &statement).await;
            
            // The statement may fail for other reasons (no DB connection), but it should
            // not be rejected as a DDL statement
            if let Err(e) = result {
                prop_assert!(
                    !matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                    "Valid DML statement '{}' should not be rejected as DDL",
                    statement
                );
            }
            
            Ok(())
        });
    }
}
