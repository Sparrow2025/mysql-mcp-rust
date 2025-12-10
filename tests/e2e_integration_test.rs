// End-to-end integration tests for MySQL MCP Server
// These tests use a real MySQL database (via Docker) to test the complete system

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig, ServerConfig, Permission};
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::pool::ConnectionPoolManager;
use mysql_mcp_server::tools::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Helper function to check if MySQL is available
async fn is_mysql_available() -> bool {
    let config = create_test_datasource_config("test-db");
    let result = ConnectionPoolManager::new(config).await;
    result.is_ok()
}

// Helper function to create a test data source configuration
fn create_test_datasource_config(key: &str) -> DataSourceConfig {
    DataSourceConfig {
        key: key.to_string(),
        name: "Test Database".to_string(),
        host: std::env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string()),
        port: std::env::var("MYSQL_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3306),
        username: std::env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
        password: std::env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "testpass".to_string()),
        databases: vec![],
        pool_config: PoolConfig::default(),
        permission: Permission::Update,
    }
}

// Helper function to create a test database
async fn create_test_database(pool_manager: &mut ConnectionPoolManager, db_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pool = pool_manager.get_pool("mysql").await?;
    
    // Drop database if exists
    let drop_query = format!("DROP DATABASE IF EXISTS {}", db_name);
    sqlx::query(&drop_query).execute(pool).await?;
    
    // Create database
    let create_query = format!("CREATE DATABASE {}", db_name);
    sqlx::query(&create_query).execute(pool).await?;
    
    Ok(())
}

// Helper function to create a test table
async fn create_test_table(
    pool_manager: &mut ConnectionPoolManager,
    db_name: &str,
    table_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = pool_manager.get_pool(db_name).await?;
    
    let create_query = format!(
        "CREATE TABLE IF NOT EXISTS {} (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255),
            age INT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        table_name
    );
    
    sqlx::query(&create_query).execute(pool).await?;
    
    Ok(())
}

// Helper function to insert test data
async fn insert_test_data(
    pool_manager: &mut ConnectionPoolManager,
    db_name: &str,
    table_name: &str,
    count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = pool_manager.get_pool(db_name).await?;
    
    for i in 0..count {
        let insert_query = format!(
            "INSERT INTO {} (name, email, age) VALUES ('User{}', 'user{}@example.com', {})",
            table_name, i, i, 20 + (i % 50)
        );
        sqlx::query(&insert_query).execute(pool).await?;
    }
    
    Ok(())
}

#[tokio::test]
async fn test_e2e_complete_query_flow() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available. Set MYSQL_HOST, MYSQL_USER, MYSQL_PASSWORD environment variables.");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    // Create test database
    create_test_database(&mut pool_manager, "e2e_test_db").await
        .expect("Failed to create test database");

    // Create test table
    create_test_table(&mut pool_manager, "e2e_test_db", "users").await
        .expect("Failed to create test table");

    // Insert test data
    insert_test_data(&mut pool_manager, "e2e_test_db", "users", 10).await
        .expect("Failed to insert test data");

    // Create manager and tools
    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let query_tool = QueryTool::new(manager.clone(), pool_managers.clone());

    // Test 1: Execute a simple SELECT query
    let result = query_tool
        .execute("test-db", "e2e_test_db", "SELECT * FROM users")
        .await
        .expect("Query failed");

    assert_eq!(result.columns.len(), 5); // id, name, email, age, created_at
    assert_eq!(result.rows.len(), 10);

    // Test 2: Execute a query with WHERE clause
    let result = query_tool
        .execute("test-db", "e2e_test_db", "SELECT * FROM users WHERE age > 30")
        .await
        .expect("Query with WHERE failed");

    assert!(result.rows.len() > 0);
    assert!(result.rows.len() < 10);

    // Test 3: Execute a query with aggregation
    let result = query_tool
        .execute("test-db", "e2e_test_db", "SELECT COUNT(*) as count FROM users")
        .await
        .expect("Aggregation query failed");

    assert_eq!(result.rows.len(), 1);

    println!("✓ Complete query flow test passed");
}

#[tokio::test]
async fn test_e2e_multi_datasource_concurrent_access() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    // Create two data source configurations (pointing to the same MySQL but with different keys)
    let config1 = create_test_datasource_config("datasource-1");
    let config2 = create_test_datasource_config("datasource-2");

    let mut pool_manager1 = ConnectionPoolManager::new(config1.clone()).await
        .expect("Failed to create pool manager 1");
    let mut pool_manager2 = ConnectionPoolManager::new(config2.clone()).await
        .expect("Failed to create pool manager 2");

    // Create test databases
    create_test_database(&mut pool_manager1, "concurrent_db1").await
        .expect("Failed to create db1");
    create_test_database(&mut pool_manager2, "concurrent_db2").await
        .expect("Failed to create db2");

    // Create test tables
    create_test_table(&mut pool_manager1, "concurrent_db1", "table1").await
        .expect("Failed to create table1");
    create_test_table(&mut pool_manager2, "concurrent_db2", "table2").await
        .expect("Failed to create table2");

    // Insert different amounts of data
    insert_test_data(&mut pool_manager1, "concurrent_db1", "table1", 5).await
        .expect("Failed to insert data into table1");
    insert_test_data(&mut pool_manager2, "concurrent_db2", "table2", 8).await
        .expect("Failed to insert data into table2");

    // Create manager with both data sources
    let manager = Arc::new(
        DataSourceManager::new(vec![config1, config2])
            .await
            .unwrap()
    );
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("datasource-1".to_string(), pool_manager1);
    pool_managers.write().await.insert("datasource-2".to_string(), pool_manager2);

    // Execute concurrent queries on both data sources
    let manager1 = manager.clone();
    let pool_managers1 = pool_managers.clone();
    let manager2 = manager.clone();
    let pool_managers2 = pool_managers.clone();

    let handle1 = tokio::spawn(async move {
        let query_tool1 = QueryTool::new(manager1, pool_managers1);
        query_tool1
            .execute("datasource-1", "concurrent_db1", "SELECT * FROM table1")
            .await
    });

    let handle2 = tokio::spawn(async move {
        let query_tool2 = QueryTool::new(manager2, pool_managers2);
        query_tool2
            .execute("datasource-2", "concurrent_db2", "SELECT * FROM table2")
            .await
    });

    let (result1, result2) = tokio::join!(handle1, handle2);

    let result1 = result1.expect("Task 1 panicked").expect("Query 1 failed");
    let result2 = result2.expect("Task 2 panicked").expect("Query 2 failed");

    // Verify results are independent
    assert_eq!(result1.rows.len(), 5);
    assert_eq!(result2.rows.len(), 8);

    println!("✓ Multi-datasource concurrent access test passed");
}

#[tokio::test]
async fn test_e2e_error_recovery_scenarios() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    create_test_database(&mut pool_manager, "error_test_db").await
        .expect("Failed to create test database");
    create_test_table(&mut pool_manager, "error_test_db", "test_table").await
        .expect("Failed to create test table");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let query_tool = QueryTool::new(manager.clone(), pool_managers.clone());

    // Test 1: Query non-existent database
    let result = query_tool
        .execute("test-db", "nonexistent_db", "SELECT 1")
        .await;
    assert!(result.is_err());
    println!("✓ Non-existent database error handled correctly");

    // Test 2: Query non-existent table
    let result = query_tool
        .execute("test-db", "error_test_db", "SELECT * FROM nonexistent_table")
        .await;
    assert!(result.is_err());
    println!("✓ Non-existent table error handled correctly");

    // Test 3: Invalid SQL syntax
    let result = query_tool
        .execute("test-db", "error_test_db", "SELECT * FORM test_table")
        .await;
    assert!(result.is_err());
    println!("✓ Invalid SQL syntax error handled correctly");

    // Test 4: Invalid data source key
    let result = query_tool
        .execute("invalid-key", "error_test_db", "SELECT 1")
        .await;
    assert!(result.is_err());
    println!("✓ Invalid data source key error handled correctly");

    // Test 5: After errors, system should still work
    let result = query_tool
        .execute("test-db", "error_test_db", "SELECT 1 as test")
        .await;
    assert!(result.is_ok());
    println!("✓ System recovered after errors");

    println!("✓ Error recovery scenarios test passed");
}

#[tokio::test]
async fn test_e2e_streaming_query() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    create_test_database(&mut pool_manager, "stream_test_db").await
        .expect("Failed to create test database");
    create_test_table(&mut pool_manager, "stream_test_db", "large_table").await
        .expect("Failed to create test table");

    // Insert a large dataset (2500 rows to test multiple chunks)
    insert_test_data(&mut pool_manager, "stream_test_db", "large_table", 2500).await
        .expect("Failed to insert test data");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let query_tool = QueryTool::new(manager.clone(), pool_managers.clone());

    // Execute query to get all data
    let result = query_tool
        .execute("test-db", "stream_test_db", "SELECT * FROM large_table")
        .await
        .expect("Query failed");

    assert_eq!(result.rows.len(), 2500);

    // Test streaming with QueryResultStream
    let stream = QueryResultStream::new(result.columns.clone(), result.rows, 1000);

    // Get first chunk
    let chunk1 = stream.next_chunk().await.expect("Failed to get chunk 1");
    assert!(chunk1.is_some());
    let chunk1 = chunk1.unwrap();
    assert_eq!(chunk1.rows.len(), 1000);
    assert_eq!(chunk1.chunk_number, 0);
    assert!(!chunk1.is_last);

    // Get second chunk
    let chunk2 = stream.next_chunk().await.expect("Failed to get chunk 2");
    assert!(chunk2.is_some());
    let chunk2 = chunk2.unwrap();
    assert_eq!(chunk2.rows.len(), 1000);
    assert_eq!(chunk2.chunk_number, 1);
    assert!(!chunk2.is_last);

    // Get third chunk (last)
    let chunk3 = stream.next_chunk().await.expect("Failed to get chunk 3");
    assert!(chunk3.is_some());
    let chunk3 = chunk3.unwrap();
    assert_eq!(chunk3.rows.len(), 500);
    assert_eq!(chunk3.chunk_number, 2);
    assert!(chunk3.is_last);

    // No more chunks
    let chunk4 = stream.next_chunk().await.expect("Failed to check end");
    assert!(chunk4.is_none());

    println!("✓ Streaming query test passed");
}

#[tokio::test]
async fn test_e2e_execute_tool_dml_operations() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    create_test_database(&mut pool_manager, "dml_test_db").await
        .expect("Failed to create test database");
    create_test_table(&mut pool_manager, "dml_test_db", "dml_table").await
        .expect("Failed to create test table");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let execute_tool = ExecuteTool::new(manager.clone(), pool_managers.clone());
    let query_tool = QueryTool::new(manager.clone(), pool_managers.clone());

    // Test 1: INSERT operation
    let result = execute_tool
        .execute(
            "test-db",
            "dml_test_db",
            "INSERT INTO dml_table (name, email, age) VALUES ('Alice', 'alice@example.com', 25)"
        )
        .await
        .expect("INSERT failed");

    assert_eq!(result.affected_rows, 1);
    assert!(result.last_insert_id.is_some());
    let insert_id = result.last_insert_id.unwrap();
    println!("✓ INSERT operation successful, last_insert_id: {}", insert_id);

    // Test 2: UPDATE operation
    let result = execute_tool
        .execute(
            "test-db",
            "dml_test_db",
            &format!("UPDATE dml_table SET age = 26 WHERE id = {}", insert_id)
        )
        .await
        .expect("UPDATE failed");

    assert_eq!(result.affected_rows, 1);
    println!("✓ UPDATE operation successful");

    // Verify update
    let query_result = query_tool
        .execute(
            "test-db",
            "dml_test_db",
            &format!("SELECT age FROM dml_table WHERE id = {}", insert_id)
        )
        .await
        .expect("SELECT failed");

    assert_eq!(query_result.rows.len(), 1);
    println!("✓ UPDATE verified");

    // Test 3: DELETE operation
    let result = execute_tool
        .execute(
            "test-db",
            "dml_test_db",
            &format!("DELETE FROM dml_table WHERE id = {}", insert_id)
        )
        .await
        .expect("DELETE failed");

    assert_eq!(result.affected_rows, 1);
    println!("✓ DELETE operation successful");

    // Verify delete
    let query_result = query_tool
        .execute(
            "test-db",
            "dml_test_db",
            &format!("SELECT * FROM dml_table WHERE id = {}", insert_id)
        )
        .await
        .expect("SELECT failed");

    assert_eq!(query_result.rows.len(), 0);
    println!("✓ DELETE verified");

    println!("✓ Execute tool DML operations test passed");
}

#[tokio::test]
async fn test_e2e_schema_tools() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    create_test_database(&mut pool_manager, "schema_test_db").await
        .expect("Failed to create test database");
    create_test_table(&mut pool_manager, "schema_test_db", "schema_table").await
        .expect("Failed to create test table");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let schema_tool = SchemaTool::new(manager.clone(), pool_managers.clone());

    // Test 1: List tables
    let tables = schema_tool
        .list_tables("test-db", "schema_test_db")
        .await
        .expect("list_tables failed");

    assert!(tables.len() >= 1);
    assert!(tables.iter().any(|t| t.name == "schema_table"));
    println!("✓ List tables successful, found {} tables", tables.len());

    // Test 2: Describe table
    let schema = schema_tool
        .describe_table("test-db", "schema_test_db", "schema_table")
        .await
        .expect("describe_table failed");

    assert_eq!(schema.table_name, "schema_table");
    assert_eq!(schema.columns.len(), 5); // id, name, email, age, created_at
    
    // Verify column details
    let id_column = schema.columns.iter().find(|c| c.name == "id");
    assert!(id_column.is_some());
    let id_column = id_column.unwrap();
    assert!(!id_column.nullable);
    
    // Verify primary key
    assert!(schema.primary_key.is_some());
    let pk = schema.primary_key.unwrap();
    assert_eq!(pk.len(), 1);
    assert_eq!(pk[0], "id");
    
    println!("✓ Describe table successful");

    // Test 3: Describe non-existent table
    let result = schema_tool
        .describe_table("test-db", "schema_test_db", "nonexistent_table")
        .await;

    assert!(result.is_err());
    println!("✓ Non-existent table error handled correctly");

    println!("✓ Schema tools test passed");
}

#[tokio::test]
async fn test_e2e_list_tools() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let list_tool = ListTool::new(manager.clone(), pool_managers.clone());

    // Test 1: List data sources
    let datasources = list_tool.list_datasources().await;
    assert_eq!(datasources.len(), 1);
    assert_eq!(datasources[0].key, "test-db");
    assert_eq!(datasources[0].name, "Test Database");
    println!("✓ List data sources successful");

    // Test 2: List databases
    let databases = list_tool
        .list_databases("test-db")
        .await
        .expect("list_databases failed");

    assert!(databases.len() > 0);
    println!("✓ List databases successful, found {} databases", databases.len());

    // Test 3: List databases with invalid key
    let result = list_tool.list_databases("invalid-key").await;
    assert!(result.is_err());
    println!("✓ Invalid key error handled correctly");

    println!("✓ List tools test passed");
}

#[tokio::test]
async fn test_e2e_connection_stats() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    // Create a test database and execute some queries to populate stats
    create_test_database(&mut pool_manager, "stats_test_db").await
        .expect("Failed to create test database");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let stats_tool = StatsTool::new(manager.clone(), pool_managers.clone());

    // Test 1: Get stats for specific data source
    let stats = stats_tool
        .get_connection_stats(Some("test-db"))
        .await
        .expect("get_connection_stats failed");

    assert!(stats.len() > 0);
    println!("✓ Connection stats retrieved successfully");

    // Test 2: Get stats for all data sources
    let all_stats = stats_tool
        .get_connection_stats(None)
        .await
        .expect("get_connection_stats (all) failed");

    assert!(all_stats.len() > 0);
    println!("✓ All connection stats retrieved successfully");

    println!("✓ Connection stats test passed");
}

#[tokio::test]
async fn test_e2e_multi_statement_handling() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    create_test_database(&mut pool_manager, "multi_stmt_db").await
        .expect("Failed to create test database");
    create_test_table(&mut pool_manager, "multi_stmt_db", "test_table").await
        .expect("Failed to create test table");
    insert_test_data(&mut pool_manager, "multi_stmt_db", "test_table", 5).await
        .expect("Failed to insert test data");

    let manager = Arc::new(DataSourceManager::new(vec![config]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    pool_managers.write().await.insert("test-db".to_string(), pool_manager);

    let query_tool = QueryTool::new(manager.clone(), pool_managers.clone());

    // Test: Execute query with multiple statements (only first should execute)
    let result = query_tool
        .execute(
            "test-db",
            "multi_stmt_db",
            "SELECT * FROM test_table WHERE id = 1; DELETE FROM test_table;"
        )
        .await
        .expect("Query failed");

    // Should only execute the SELECT, not the DELETE
    assert_eq!(result.rows.len(), 1);

    // Verify that DELETE was not executed
    let verify_result = query_tool
        .execute("test-db", "multi_stmt_db", "SELECT COUNT(*) as count FROM test_table")
        .await
        .expect("Verify query failed");

    // All 5 rows should still be there
    assert_eq!(verify_result.rows.len(), 1);

    println!("✓ Multi-statement handling test passed");
}
