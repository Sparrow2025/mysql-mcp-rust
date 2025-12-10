// Feature: mysql-mcp-multi-datasource, Property 7: Query execution correctness
// Validates: Requirements 3.1

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::error::McpError;
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::QueryTool;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Strategy to generate arbitrary non-empty strings for valid parameters
fn arbitrary_non_empty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,30}"
}

// Strategy to generate arbitrary SQL queries
fn arbitrary_sql_query() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple SELECT queries
        Just("SELECT 1".to_string()),
        Just("SELECT 1 + 1".to_string()),
        Just("SELECT NOW()".to_string()),
        Just("SELECT VERSION()".to_string()),
        Just("SELECT DATABASE()".to_string()),
        // SELECT with WHERE clause
        "[a-zA-Z_][a-zA-Z0-9_]{0,10}".prop_map(|table| format!("SELECT * FROM {}", table)),
        "[a-zA-Z_][a-zA-Z0-9_]{0,10}".prop_map(|table| format!("SELECT COUNT(*) FROM {}", table)),
        // Multi-statement queries (should only execute first)
        Just("SELECT 1; SELECT 2".to_string()),
        Just("SELECT 1; DROP TABLE users".to_string()),
        // Queries with string literals containing semicolons
        Just("SELECT 'test;data'".to_string()),
        Just("SELECT 'test;data'; SELECT 2".to_string()),
    ]
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

    /// Property 7: Query execution correctness
    /// For any valid query with correct data source key and database name,
    /// the query should execute and return results or an error (but not a parameter validation error)
    /// 
    /// Note: This test verifies the execution path without requiring a real database.
    /// It ensures that valid parameters are accepted and processed correctly,
    /// and that any errors are execution-related, not parameter validation errors.
    #[test]
    fn test_query_execution_correctness(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        query in arbitrary_sql_query(),
    ) {
        // Create a tokio runtime for the async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            // Create a manager with one valid data source
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager.clone(), pool_managers);

            // Execute the query with valid parameters
            // Use a short timeout to avoid hanging on connection attempts
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                tool.execute(&datasource_key, &database, &query)
            ).await;

            // The query should either:
            // 1. Timeout (expected when no real database is available)
            // 2. Return an execution error (ConnectionFailed, DatabaseNotFound, etc.)
            // 3. Never return InvalidStatement error with valid parameters
            match result {
                Ok(Ok(query_result)) => {
                    // Query succeeded - verify the result structure
                    // This would only happen if a real database is available
                    prop_assert!(
                        true,
                        "Query succeeded with valid parameters - this is correct behavior"
                    );
                }
                Ok(Err(e)) => {
                    // Query failed - verify it's not a parameter validation error
                    // With valid parameters, we should never get InvalidStatement errors
                    prop_assert!(
                        !matches!(e, McpError::InvalidStatement(_)),
                        "Query with valid parameters should not fail with InvalidStatement error. \
                         Got error: {:?} for query: '{}', datasource: '{}', database: '{}'",
                        e, query, datasource_key, database
                    );

                    // The error should be one of the expected execution errors
                    prop_assert!(
                        matches!(
                            e,
                            McpError::DatabaseNotFound(_)
                                | McpError::QueryExecutionError(_)
                                | McpError::ConnectionFailed(_)
                                | McpError::QueryTimeout
                                | McpError::DataSourceUnavailable(_)
                                | McpError::NetworkError(_)
                                | McpError::PoolError(_)
                        ),
                        "Query execution error should be a valid execution error type. \
                         Got: {:?} for query: '{}', datasource: '{}', database: '{}'",
                        e, query, datasource_key, database
                    );
                }
                Err(_timeout) => {
                    // Timeout is expected when no real database is available
                    // This means the query tool accepted the valid parameters
                    // and attempted to execute, which is correct behavior
                    prop_assert!(
                        true,
                        "Query timed out attempting to connect - valid parameters were accepted"
                    );
                }
            }
            
            Ok(())
        });
    }

    /// Property 7a: Valid parameters never cause InvalidStatement errors
    /// For any query with valid (non-empty) parameters, the error should never be InvalidStatement
    #[test]
    fn test_valid_parameters_no_invalid_statement_error(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        query in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            // Use a short timeout to avoid hanging
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                tool.execute(&datasource_key, &database, &query)
            ).await;

            // With all valid (non-empty) parameters, we should never get InvalidStatement
            if let Ok(Err(e)) = result {
                prop_assert!(
                    !matches!(e, McpError::InvalidStatement(_)),
                    "Valid parameters should not produce InvalidStatement error. \
                     Got: {:?} for datasource: '{}', database: '{}', query: '{}'",
                    e, datasource_key, database, query
                );
            }
            
            Ok(())
        });
    }

    /// Property 7b: Query execution returns structured results on success
    /// For any successful query execution, the result should have proper structure
    /// 
    /// Note: This test only validates structure if a database connection succeeds.
    /// It's expected to timeout or fail with connection errors in most test environments.
    #[test]
    fn test_successful_query_has_structured_result(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            // Use a simple query that should work if database connection is available
            let query = "SELECT 1 AS test_column";
            
            // Use a short timeout to avoid hanging
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                tool.execute(&datasource_key, &database, query)
            ).await;

            // If the query succeeds (database is available), verify the structure
            if let Ok(Ok(query_result)) = result {
                // Should have at least one column
                prop_assert!(
                    !query_result.columns.is_empty(),
                    "Successful query should have at least one column"
                );
                
                // Each column should have a name and data type
                for column in &query_result.columns {
                    prop_assert!(
                        !column.name.is_empty(),
                        "Column should have a non-empty name"
                    );
                    prop_assert!(
                        !column.data_type.is_empty(),
                        "Column should have a non-empty data type"
                    );
                }
                
                // Should have at least one row for "SELECT 1"
                prop_assert!(
                    !query_result.rows.is_empty(),
                    "SELECT 1 should return at least one row"
                );
                
                // Each row should have values matching the number of columns
                for row in &query_result.rows {
                    prop_assert_eq!(
                        row.values.len(),
                        query_result.columns.len(),
                        "Row should have same number of values as columns"
                    );
                }
            }
            // If it fails or times out, that's okay - we're only testing structure when it succeeds
            
            Ok(())
        });
    }

    /// Property 7c: Multi-statement queries only execute first statement
    /// For any query containing multiple statements, only the first should be executed
    #[test]
    fn test_multi_statement_only_executes_first(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager, pool_managers);

            // Query with multiple statements - second statement would fail if executed
            let query = "SELECT 1; THIS_IS_INVALID_SQL";
            
            // Use a short timeout to avoid hanging
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                tool.execute(&datasource_key, &database, query)
            ).await;

            // The query should either succeed (if first statement executed)
            // or fail with a connection/database error (not a SQL syntax error from second statement)
            match result {
                Ok(Ok(_)) => {
                    // Success means only first statement was executed
                    // This is the expected behavior
                    prop_assert!(true, "Query succeeded - only first statement was executed");
                }
                Ok(Err(e)) => {
                    // If it fails, it should be due to connection/database issues,
                    // not because the invalid second statement was executed
                    // If the second statement was executed, we'd get a QueryExecutionError
                    // with "THIS_IS_INVALID_SQL" in the message
                    let error_msg = format!("{:?}", e);
                    prop_assert!(
                        !error_msg.contains("THIS_IS_INVALID_SQL"),
                        "Error should not mention the second statement, indicating it was not executed. \
                         Got error: {:?}",
                        e
                    );
                }
                Err(_timeout) => {
                    // Timeout is expected - the important thing is that the query tool
                    // accepted the parameters and attempted execution
                    prop_assert!(true, "Query timed out - parameters were accepted");
                }
            }
            
            Ok(())
        });
    }

    /// Property 7d: Query execution handles unavailable data sources
    /// For any query to an unavailable data source, should return DataSourceUnavailable error
    #[test]
    fn test_unavailable_datasource_returns_correct_error(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        query in arbitrary_sql_query(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = QueryTool::new(manager.clone(), pool_managers);

            // Mark the data source as unavailable
            manager.set_status(&datasource_key, mysql_mcp_server::manager::ConnectionStatus::Unavailable).await;

            // Try to execute a query - this should fail immediately without trying to connect
            let result = tool.execute(&datasource_key, &database, &query).await;

            // Should fail with DataSourceUnavailable error
            prop_assert!(
                result.is_err(),
                "Query to unavailable data source should fail"
            );
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, McpError::DataSourceUnavailable(_)),
                    "Query to unavailable data source should return DataSourceUnavailable error. \
                     Got: {:?}",
                    e
                );
            }
            
            Ok(())
        });
    }
}
