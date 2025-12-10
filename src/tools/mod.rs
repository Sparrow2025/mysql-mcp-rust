use crate::error::{McpError, Result};
use crate::manager::{DataSourceManager, DataSourceInfo};
use crate::pool::ConnectionPoolManager;
use serde::{Deserialize, Serialize};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Mutex};

/// Query tool for executing SQL queries
pub struct QueryTool {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
}

impl QueryTool {
    /// Create a new query tool
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    ) -> Self {
        Self {
            manager,
            pool_managers,
        }
    }

    /// Execute a SQL query
    pub async fn execute(
        &self,
        datasource_key: &str,
        database: &str,
        query: &str,
    ) -> Result<QueryResult> {
        // Validate parameters
        if datasource_key.is_empty() {
            return Err(McpError::InvalidStatement(
                "Data source key is required".to_string(),
            ));
        }

        if database.is_empty() {
            return Err(McpError::InvalidStatement(
                "Database name is required".to_string(),
            ));
        }

        if query.trim().is_empty() {
            return Err(McpError::InvalidStatement("Query is required".to_string()));
        }

        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check query permission
        self.manager.check_query_permission(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Extract only the first statement if multiple statements are present
        let first_statement = extract_first_statement(query);

        tracing::info!(
            datasource_key = %datasource_key,
            database = %database,
            query_length = first_statement.len(),
            "Executing query"
        );

        // Get or create pool manager for this data source
        let mut pool_managers = self.pool_managers.write().await;
        if !pool_managers.contains_key(datasource_key) {
            let config = self
                .manager
                .get_source(datasource_key)
                .ok_or_else(|| McpError::InvalidDataSourceKey(datasource_key.to_string()))?;
            let pool_manager = ConnectionPoolManager::new((*config).clone()).await?;
            pool_managers.insert(datasource_key.to_string(), pool_manager);
        }

        let pool_manager = pool_managers.get_mut(datasource_key).unwrap();

        // Execute query with timeout
        let start = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            execute_query(pool_manager, database, first_statement),
        )
        .await;

        let execution_time = start.elapsed();

        match result {
            Ok(Ok(query_result)) => {
                tracing::info!(
                    datasource_key = %datasource_key,
                    database = %database,
                    rows = query_result.rows.len(),
                    execution_time_ms = execution_time.as_millis(),
                    "Query executed successfully"
                );
                Ok(query_result)
            }
            Ok(Err(e)) => {
                tracing::error!(
                    datasource_key = %datasource_key,
                    database = %database,
                    error = %e.sanitize(),
                    execution_time_ms = execution_time.as_millis(),
                    "Query execution failed"
                );
                Err(e)
            }
            Err(_) => {
                tracing::error!(
                    datasource_key = %datasource_key,
                    database = %database,
                    "Query execution timed out after 30 seconds"
                );
                Err(McpError::QueryTimeout)
            }
        }
    }
}

/// Extract the first SQL statement from a query string
/// Handles multiple statements separated by semicolons
fn extract_first_statement(query: &str) -> &str {
    let trimmed = query.trim();

    // Find the first semicolon that's not inside a string literal
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    for (i, ch) in trimmed.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => escape_next = true,
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            ';' if !in_single_quote && !in_double_quote => {
                return trimmed[..i].trim();
            }
            _ => {}
        }
    }

    // No semicolon found or only one statement
    trimmed
}

/// Execute a query and return the results
async fn execute_query(
    pool_manager: &mut ConnectionPoolManager,
    database: &str,
    query: &str,
) -> Result<QueryResult> {
    let pool = pool_manager.get_pool(database).await?;

    // Execute the query
    let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(query)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            // Check if it's a database not found error
            let error_msg = e.to_string();
            if error_msg.contains("Unknown database") {
                McpError::DatabaseNotFound(database.to_string())
            } else {
                McpError::QueryExecutionError(error_msg)
            }
        })?;

    // Extract column metadata from the first row (if any)
    let columns = if let Some(first_row) = rows.first() {
        first_row
            .columns()
            .iter()
            .map(|col| ColumnMetadata {
                name: col.name().to_string(),
                data_type: col.type_info().name().to_string(),
                nullable: true, // MySQL doesn't provide this info easily from query results
            })
            .collect()
    } else {
        vec![]
    };

    // Convert rows to our format
    let result_rows: Vec<QueryRow> = rows
        .iter()
        .map(|row| {
            let values: Vec<serde_json::Value> = row
                .columns()
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    // Try to extract the value based on the column type
                    extract_value(row, i, col.type_info().name())
                })
                .collect();

            QueryRow { values }
        })
        .collect();

    Ok(QueryResult {
        columns,
        rows: result_rows,
        affected_rows: 0, // SELECT queries don't have affected rows
    })
}

/// Extract a value from a row at the given index
fn extract_value(row: &sqlx::mysql::MySqlRow, index: usize, type_name: &str) -> serde_json::Value {
    use sqlx::Row;

    // Check if the value is NULL
    if row.try_get_raw(index).map(|v| v.is_null()).unwrap_or(true) {
        return serde_json::Value::Null;
    }

    // Try to extract based on type
    match type_name.to_uppercase().as_str() {
        "TINYINT" | "SMALLINT" | "INT" | "MEDIUMINT" | "INTEGER" => {
            row.try_get::<i32, _>(index)
                .ok()
                .map(|v| serde_json::json!(v))
                .unwrap_or(serde_json::Value::Null)
        }
        "BIGINT" => row
            .try_get::<i64, _>(index)
            .ok()
            .map(|v| serde_json::json!(v))
            .unwrap_or(serde_json::Value::Null),
        "FLOAT" | "DOUBLE" | "DECIMAL" | "NUMERIC" => row
            .try_get::<f64, _>(index)
            .ok()
            .map(|v| serde_json::json!(v))
            .unwrap_or(serde_json::Value::Null),
        "BOOLEAN" | "BOOL" => row
            .try_get::<bool, _>(index)
            .ok()
            .map(|v| serde_json::json!(v))
            .unwrap_or(serde_json::Value::Null),
        "DATE" | "DATETIME" | "TIMESTAMP" => row
            .try_get::<chrono::NaiveDateTime, _>(index)
            .ok()
            .map(|v| serde_json::json!(v.to_string()))
            .or_else(|| {
                row.try_get::<chrono::NaiveDate, _>(index)
                    .ok()
                    .map(|v| serde_json::json!(v.to_string()))
            })
            .unwrap_or(serde_json::Value::Null),
        "TIME" => row
            .try_get::<chrono::NaiveTime, _>(index)
            .ok()
            .map(|v| serde_json::json!(v.to_string()))
            .unwrap_or(serde_json::Value::Null),
        _ => {
            // Default to string for all other types
            row.try_get::<String, _>(index)
                .ok()
                .map(|v| serde_json::json!(v))
                .unwrap_or(serde_json::Value::Null)
        }
    }
}

/// Column metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMetadata {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

/// A row in the query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRow {
    pub values: Vec<serde_json::Value>,
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnMetadata>,
    pub rows: Vec<QueryRow>,
    pub affected_rows: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DataSourceConfig, PoolConfig};

    #[test]
    fn test_extract_first_statement_single() {
        let query = "SELECT * FROM users";
        assert_eq!(extract_first_statement(query), "SELECT * FROM users");
    }

    #[test]
    fn test_extract_first_statement_multiple() {
        let query = "SELECT * FROM users; DELETE FROM users;";
        assert_eq!(extract_first_statement(query), "SELECT * FROM users");
    }

    #[test]
    fn test_extract_first_statement_with_semicolon_in_string() {
        let query = "SELECT * FROM users WHERE name = 'test;data'; DELETE FROM users;";
        assert_eq!(
            extract_first_statement(query),
            "SELECT * FROM users WHERE name = 'test;data'"
        );
    }

    #[test]
    fn test_extract_first_statement_with_double_quotes() {
        let query = r#"SELECT * FROM users WHERE name = "test;data"; DELETE FROM users;"#;
        assert_eq!(
            extract_first_statement(query),
            r#"SELECT * FROM users WHERE name = "test;data""#
        );
    }

    #[test]
    fn test_extract_first_statement_with_escaped_quotes() {
        let query = r"SELECT * FROM users WHERE name = 'test\'s;data'; DELETE FROM users;";
        assert_eq!(
            extract_first_statement(query),
            r"SELECT * FROM users WHERE name = 'test\'s;data'"
        );
    }

    #[test]
    fn test_extract_first_statement_no_semicolon() {
        let query = "SELECT * FROM users WHERE id = 1";
        assert_eq!(
            extract_first_statement(query),
            "SELECT * FROM users WHERE id = 1"
        );
    }

    #[test]
    fn test_extract_first_statement_whitespace() {
        let query = "  SELECT * FROM users  ; DELETE FROM users;  ";
        assert_eq!(extract_first_statement(query), "SELECT * FROM users");
    }

    fn create_test_config(key: &str) -> DataSourceConfig {
        DataSourceConfig {
            key: key.to_string(),
            name: "Test Database".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "test".to_string(),
            password: "test".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        }
    }

    #[tokio::test]
    async fn test_query_tool_validates_empty_datasource_key() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = QueryTool::new(manager, pool_managers);

        let result = tool.execute("", "testdb", "SELECT 1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_query_tool_validates_empty_database() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = QueryTool::new(manager, pool_managers);

        let result = tool.execute("test-db", "", "SELECT 1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_query_tool_validates_empty_query() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = QueryTool::new(manager, pool_managers);

        let result = tool.execute("test-db", "testdb", "").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_query_tool_validates_invalid_datasource_key() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = QueryTool::new(manager, pool_managers);

        let result = tool.execute("invalid-key", "testdb", "SELECT 1").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpError::InvalidDataSourceKey(_)
        ));
    }

    #[test]
    fn test_is_ddl_statement_create() {
        assert!(is_ddl_statement("CREATE TABLE users (id INT)"));
        assert!(is_ddl_statement("create table users (id INT)"));
        assert!(is_ddl_statement("  CREATE TABLE users (id INT)"));
    }

    #[test]
    fn test_is_ddl_statement_alter() {
        assert!(is_ddl_statement("ALTER TABLE users ADD COLUMN name VARCHAR(255)"));
        assert!(is_ddl_statement("alter table users add column name varchar(255)"));
    }

    #[test]
    fn test_is_ddl_statement_drop() {
        assert!(is_ddl_statement("DROP TABLE users"));
        assert!(is_ddl_statement("drop table users"));
    }

    #[test]
    fn test_is_ddl_statement_truncate() {
        assert!(is_ddl_statement("TRUNCATE TABLE users"));
        assert!(is_ddl_statement("truncate table users"));
    }

    #[test]
    fn test_is_ddl_statement_rename() {
        assert!(is_ddl_statement("RENAME TABLE users TO customers"));
        assert!(is_ddl_statement("rename table users to customers"));
    }

    #[test]
    fn test_is_not_ddl_statement() {
        assert!(!is_ddl_statement("SELECT * FROM users"));
        assert!(!is_ddl_statement("INSERT INTO users VALUES (1, 'test')"));
        assert!(!is_ddl_statement("UPDATE users SET name = 'test'"));
        assert!(!is_ddl_statement("DELETE FROM users WHERE id = 1"));
    }

    #[tokio::test]
    async fn test_execute_tool_validates_empty_datasource_key() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = ExecuteTool::new(manager, pool_managers);

        let result = tool.execute("", "testdb", "INSERT INTO users VALUES (1)").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_execute_tool_validates_empty_database() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = ExecuteTool::new(manager, pool_managers);

        let result = tool.execute("test-db", "", "INSERT INTO users VALUES (1)").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_execute_tool_validates_empty_statement() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = ExecuteTool::new(manager, pool_managers);

        let result = tool.execute("test-db", "testdb", "").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_execute_tool_validates_invalid_datasource_key() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = ExecuteTool::new(manager, pool_managers);

        let result = tool.execute("invalid-key", "testdb", "INSERT INTO users VALUES (1)").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpError::InvalidDataSourceKey(_)
        ));
    }

    #[tokio::test]
    async fn test_execute_tool_rejects_ddl_statements() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = ExecuteTool::new(manager, pool_managers);

        // Test CREATE - should be rejected due to lack of DDL permission (default is query-only)
        let result = tool.execute("test-db", "testdb", "CREATE TABLE users (id INT)").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

        // Test ALTER - should be rejected due to lack of DDL permission
        let result = tool.execute("test-db", "testdb", "ALTER TABLE users ADD COLUMN name VARCHAR(255)").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

        // Test DROP - should be rejected due to lack of DDL permission
        let result = tool.execute("test-db", "testdb", "DROP TABLE users").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

        // Test TRUNCATE - should be rejected due to lack of DDL permission
        let result = tool.execute("test-db", "testdb", "TRUNCATE TABLE users").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

        // Test RENAME - should be rejected due to lack of DDL permission
        let result = tool.execute("test-db", "testdb", "RENAME TABLE users TO customers").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));
    }
}

/// Stream handler for query results
/// Provides chunked streaming of large result sets
pub struct QueryResultStream {
    rows: Arc<Mutex<Vec<QueryRow>>>,
    columns: Vec<ColumnMetadata>,
    chunk_size: usize,
    current_position: Arc<Mutex<usize>>,
    cancelled: Arc<Mutex<bool>>,
    stream_id: String,
}

impl QueryResultStream {
    /// Create a new query result stream
    pub fn new(
        columns: Vec<ColumnMetadata>,
        rows: Vec<QueryRow>,
        chunk_size: usize,
    ) -> Self {
        let stream_id = uuid::Uuid::new_v4().to_string();
        
        tracing::debug!(
            stream_id = %stream_id,
            total_rows = rows.len(),
            chunk_size = chunk_size,
            "Creating new query result stream"
        );

        Self {
            rows: Arc::new(Mutex::new(rows)),
            columns,
            chunk_size,
            current_position: Arc::new(Mutex::new(0)),
            cancelled: Arc::new(Mutex::new(false)),
            stream_id,
        }
    }

    /// Get the next chunk of rows
    /// Returns None when all rows have been consumed or stream is cancelled
    pub async fn next_chunk(&self) -> Result<Option<QueryResultChunk>> {
        // Check if stream is cancelled
        let is_cancelled = *self.cancelled.lock().await;
        if is_cancelled {
            tracing::debug!(
                stream_id = %self.stream_id,
                "Stream is cancelled, returning None"
            );
            return Err(McpError::StreamCancelled);
        }

        let mut position = self.current_position.lock().await;
        let rows = self.rows.lock().await;

        // Check if we've reached the end
        if *position >= rows.len() {
            tracing::debug!(
                stream_id = %self.stream_id,
                total_rows = rows.len(),
                "Stream completed, all rows consumed"
            );
            return Ok(None);
        }

        // Calculate chunk boundaries
        let start = *position;
        let end = std::cmp::min(start + self.chunk_size, rows.len());
        let chunk_rows = rows[start..end].to_vec();
        let chunk_number = start / self.chunk_size;
        let is_last = end >= rows.len();

        // Update position
        *position = end;

        tracing::debug!(
            stream_id = %self.stream_id,
            chunk_number = chunk_number,
            chunk_size = chunk_rows.len(),
            is_last = is_last,
            "Returning chunk"
        );

        Ok(Some(QueryResultChunk {
            columns: self.columns.clone(),
            rows: chunk_rows,
            chunk_number,
            is_last,
            total_rows: rows.len(),
        }))
    }

    /// Cancel the stream and clean up resources
    pub async fn cancel(&self) -> Result<()> {
        let mut cancelled = self.cancelled.lock().await;
        *cancelled = true;

        tracing::info!(
            stream_id = %self.stream_id,
            "Stream cancelled"
        );

        // Clear the rows to free memory
        let mut rows = self.rows.lock().await;
        rows.clear();

        Ok(())
    }

    /// Check if the stream is cancelled
    pub async fn is_cancelled(&self) -> bool {
        *self.cancelled.lock().await
    }

    /// Get the stream ID
    pub fn stream_id(&self) -> &str {
        &self.stream_id
    }

    /// Get the total number of rows in the stream
    pub async fn total_rows(&self) -> usize {
        let rows = self.rows.lock().await;
        rows.len()
    }

    /// Get the current position in the stream
    pub async fn current_position(&self) -> usize {
        *self.current_position.lock().await
    }

    /// Get the column metadata
    pub fn columns(&self) -> &[ColumnMetadata] {
        &self.columns
    }
}

/// A chunk of query results from a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResultChunk {
    pub columns: Vec<ColumnMetadata>,
    pub rows: Vec<QueryRow>,
    pub chunk_number: usize,
    pub is_last: bool,
    pub total_rows: usize,
}

/// Manager for concurrent query streams
/// Ensures isolation between different streams
pub struct StreamManager {
    streams: Arc<RwLock<HashMap<String, Arc<QueryResultStream>>>>,
}

impl StreamManager {
    /// Create a new stream manager
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new stream
    pub async fn register_stream(&self, stream: QueryResultStream) -> String {
        let stream_id = stream.stream_id().to_string();
        let mut streams = self.streams.write().await;
        streams.insert(stream_id.clone(), Arc::new(stream));

        tracing::info!(
            stream_id = %stream_id,
            active_streams = streams.len(),
            "Stream registered"
        );

        stream_id
    }

    /// Get a stream by ID
    pub async fn get_stream(&self, stream_id: &str) -> Option<Arc<QueryResultStream>> {
        let streams = self.streams.read().await;
        streams.get(stream_id).cloned()
    }

    /// Remove a stream
    pub async fn remove_stream(&self, stream_id: &str) -> Result<()> {
        let mut streams = self.streams.write().await;
        
        if let Some(stream) = streams.remove(stream_id) {
            // Cancel the stream to clean up resources
            stream.cancel().await?;
            
            tracing::info!(
                stream_id = %stream_id,
                remaining_streams = streams.len(),
                "Stream removed"
            );
        }

        Ok(())
    }

    /// Get the number of active streams
    pub async fn active_stream_count(&self) -> usize {
        let streams = self.streams.read().await;
        streams.len()
    }

    /// Cancel all streams
    pub async fn cancel_all(&self) -> Result<()> {
        let mut streams = self.streams.write().await;
        
        for (stream_id, stream) in streams.iter() {
            tracing::info!(
                stream_id = %stream_id,
                "Cancelling stream"
            );
            stream.cancel().await?;
        }
        
        streams.clear();
        
        tracing::info!("All streams cancelled");
        
        Ok(())
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute tool for executing DML statements (INSERT, UPDATE, DELETE)
pub struct ExecuteTool {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
}

impl ExecuteTool {
    /// Create a new execute tool
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    ) -> Self {
        Self {
            manager,
            pool_managers,
        }
    }

    /// Execute a DML statement (INSERT, UPDATE, DELETE)
    pub async fn execute(
        &self,
        datasource_key: &str,
        database: &str,
        statement: &str,
    ) -> Result<ExecuteResult> {
        // Validate parameters
        if datasource_key.is_empty() {
            return Err(McpError::InvalidStatement(
                "Data source key is required".to_string(),
            ));
        }

        if database.is_empty() {
            return Err(McpError::InvalidStatement(
                "Database name is required".to_string(),
            ));
        }

        if statement.trim().is_empty() {
            return Err(McpError::InvalidStatement("Statement is required".to_string()));
        }

        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Check if the statement is a DDL statement
        if is_ddl_statement(statement) {
            // Check DDL permission
            self.manager.check_ddl_permission(datasource_key)?;
        } else {
            // Check update permission for DML statements
            self.manager.check_update_permission(datasource_key)?;
        }

        tracing::info!(
            datasource_key = %datasource_key,
            database = %database,
            statement_length = statement.len(),
            "Executing DML statement"
        );

        // Get or create pool manager for this data source
        let mut pool_managers = self.pool_managers.write().await;
        if !pool_managers.contains_key(datasource_key) {
            let config = self
                .manager
                .get_source(datasource_key)
                .ok_or_else(|| McpError::InvalidDataSourceKey(datasource_key.to_string()))?;
            let pool_manager = ConnectionPoolManager::new((*config).clone()).await?;
            pool_managers.insert(datasource_key.to_string(), pool_manager);
        }

        let pool_manager = pool_managers.get_mut(datasource_key).unwrap();

        // Execute statement with timeout
        let start = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            execute_dml_statement(pool_manager, database, statement),
        )
        .await;

        let execution_time = start.elapsed();

        match result {
            Ok(Ok(execute_result)) => {
                tracing::info!(
                    datasource_key = %datasource_key,
                    database = %database,
                    affected_rows = execute_result.affected_rows,
                    last_insert_id = ?execute_result.last_insert_id,
                    execution_time_ms = execution_time.as_millis(),
                    "DML statement executed successfully"
                );
                Ok(execute_result)
            }
            Ok(Err(e)) => {
                tracing::error!(
                    datasource_key = %datasource_key,
                    database = %database,
                    error = %e.sanitize(),
                    execution_time_ms = execution_time.as_millis(),
                    "DML statement execution failed"
                );
                Err(e)
            }
            Err(_) => {
                tracing::error!(
                    datasource_key = %datasource_key,
                    database = %database,
                    "DML statement execution timed out after 30 seconds"
                );
                Err(McpError::QueryTimeout)
            }
        }
    }
}

/// Check if a statement is a DDL statement
/// DDL statements include: CREATE, ALTER, DROP, TRUNCATE, RENAME
fn is_ddl_statement(statement: &str) -> bool {
    let trimmed = statement.trim().to_uppercase();
    
    // Check for DDL keywords at the start of the statement
    trimmed.starts_with("CREATE ")
        || trimmed.starts_with("ALTER ")
        || trimmed.starts_with("DROP ")
        || trimmed.starts_with("TRUNCATE ")
        || trimmed.starts_with("RENAME ")
}

/// Execute a DML statement and return the result
async fn execute_dml_statement(
    pool_manager: &mut ConnectionPoolManager,
    database: &str,
    statement: &str,
) -> Result<ExecuteResult> {
    let pool = pool_manager.get_pool(database).await?;

    // Execute the statement
    let result = sqlx::query(statement)
        .execute(pool)
        .await
        .map_err(|e| {
            // Check if it's a database not found error
            let error_msg = e.to_string();
            if error_msg.contains("Unknown database") {
                McpError::DatabaseNotFound(database.to_string())
            } else {
                McpError::QueryExecutionError(error_msg)
            }
        })?;

    Ok(ExecuteResult {
        affected_rows: result.rows_affected(),
        last_insert_id: if result.last_insert_id() > 0 {
            Some(result.last_insert_id())
        } else {
            None
        },
    })
}

/// Result of executing a DML statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub affected_rows: u64,
    pub last_insert_id: Option<u64>,
}

/// Schema tool for retrieving database schema information
pub struct SchemaTool {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
}

impl SchemaTool {
    /// Create a new schema tool
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    ) -> Self {
        Self {
            manager,
            pool_managers,
        }
    }

    /// List all tables in a database
    pub async fn list_tables(
        &self,
        datasource_key: &str,
        database: &str,
    ) -> Result<Vec<TableInfo>> {
        // Validate parameters
        if datasource_key.is_empty() {
            return Err(McpError::InvalidStatement(
                "Data source key is required".to_string(),
            ));
        }

        if database.is_empty() {
            return Err(McpError::InvalidStatement(
                "Database name is required".to_string(),
            ));
        }

        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        tracing::info!(
            datasource_key = %datasource_key,
            database = %database,
            "Listing tables"
        );

        // Get or create pool manager for this data source
        let mut pool_managers = self.pool_managers.write().await;
        if !pool_managers.contains_key(datasource_key) {
            let config = self
                .manager
                .get_source(datasource_key)
                .ok_or_else(|| McpError::InvalidDataSourceKey(datasource_key.to_string()))?;
            let pool_manager = ConnectionPoolManager::new((*config).clone()).await?;
            pool_managers.insert(datasource_key.to_string(), pool_manager);
        }

        let pool_manager = pool_managers.get_mut(datasource_key).unwrap();
        let pool = pool_manager.get_pool(database).await?;

        // Query to get table information
        let query = format!(
            "SELECT 
                TABLE_NAME as name,
                TABLE_ROWS as row_count,
                DATA_LENGTH + INDEX_LENGTH as size_bytes,
                ENGINE as engine
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = '{}'
             ORDER BY TABLE_NAME",
            database.replace('\'', "''") // Escape single quotes
        );

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                let error_msg = e.to_string();
                if error_msg.contains("Unknown database") {
                    McpError::DatabaseNotFound(database.to_string())
                } else {
                    McpError::QueryExecutionError(error_msg)
                }
            })?;

        let tables: Vec<TableInfo> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                TableInfo {
                    name: row.try_get("name").unwrap_or_default(),
                    row_count: row.try_get::<Option<u64>, _>("row_count").ok().flatten(),
                    size_bytes: row.try_get::<Option<u64>, _>("size_bytes").ok().flatten(),
                    engine: row.try_get("engine").ok(),
                }
            })
            .collect();

        tracing::info!(
            datasource_key = %datasource_key,
            database = %database,
            table_count = tables.len(),
            "Listed tables successfully"
        );

        Ok(tables)
    }

    /// Describe a specific table (get its schema)
    pub async fn describe_table(
        &self,
        datasource_key: &str,
        database: &str,
        table: &str,
    ) -> Result<TableSchema> {
        // Validate parameters
        if datasource_key.is_empty() {
            return Err(McpError::InvalidStatement(
                "Data source key is required".to_string(),
            ));
        }

        if database.is_empty() {
            return Err(McpError::InvalidStatement(
                "Database name is required".to_string(),
            ));
        }

        if table.is_empty() {
            return Err(McpError::InvalidStatement(
                "Table name is required".to_string(),
            ));
        }

        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        tracing::info!(
            datasource_key = %datasource_key,
            database = %database,
            table = %table,
            "Describing table"
        );

        // Get or create pool manager for this data source
        let mut pool_managers = self.pool_managers.write().await;
        if !pool_managers.contains_key(datasource_key) {
            let config = self
                .manager
                .get_source(datasource_key)
                .ok_or_else(|| McpError::InvalidDataSourceKey(datasource_key.to_string()))?;
            let pool_manager = ConnectionPoolManager::new((*config).clone()).await?;
            pool_managers.insert(datasource_key.to_string(), pool_manager);
        }

        let pool_manager = pool_managers.get_mut(datasource_key).unwrap();
        let pool = pool_manager.get_pool(database).await?;

        // First, check if the table exists
        let table_exists_query = format!(
            "SELECT COUNT(*) as count FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'",
            database.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let exists_row: (i64,) = sqlx::query_as(&table_exists_query)
            .fetch_one(pool)
            .await
            .map_err(|e| McpError::QueryExecutionError(e.to_string()))?;

        if exists_row.0 == 0 {
            return Err(McpError::TableNotFound(format!(
                "Table '{}' not found in database '{}'",
                table, database
            )));
        }

        // Get column information
        let columns = self.get_columns(pool, database, table).await?;

        // Get primary key information
        let primary_key = self.get_primary_key(pool, database, table).await?;

        // Get foreign key information
        let foreign_keys = self.get_foreign_keys(pool, database, table).await?;

        // Get index information
        let indexes = self.get_indexes(pool, database, table).await?;

        tracing::info!(
            datasource_key = %datasource_key,
            database = %database,
            table = %table,
            columns = columns.len(),
            "Described table successfully"
        );

        Ok(TableSchema {
            table_name: table.to_string(),
            columns,
            primary_key,
            foreign_keys,
            indexes,
        })
    }

    /// Get column information for a table
    async fn get_columns(
        &self,
        pool: &sqlx::Pool<sqlx::MySql>,
        database: &str,
        table: &str,
    ) -> Result<Vec<ColumnSchema>> {
        let query = format!(
            "SELECT 
                COLUMN_NAME as name,
                COLUMN_TYPE as data_type,
                IS_NULLABLE as nullable,
                COLUMN_DEFAULT as default_value,
                COLUMN_COMMENT as comment
             FROM information_schema.COLUMNS
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'
             ORDER BY ORDINAL_POSITION",
            database.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| McpError::QueryExecutionError(e.to_string()))?;

        let columns: Vec<ColumnSchema> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                ColumnSchema {
                    name: row.try_get("name").unwrap_or_default(),
                    data_type: row.try_get("data_type").unwrap_or_default(),
                    nullable: row
                        .try_get::<String, _>("nullable")
                        .map(|s| s == "YES")
                        .unwrap_or(false),
                    default_value: row.try_get("default_value").ok(),
                    comment: row.try_get("comment").ok(),
                }
            })
            .collect();

        Ok(columns)
    }

    /// Get primary key information for a table
    async fn get_primary_key(
        &self,
        pool: &sqlx::Pool<sqlx::MySql>,
        database: &str,
        table: &str,
    ) -> Result<Option<Vec<String>>> {
        let query = format!(
            "SELECT COLUMN_NAME
             FROM information_schema.KEY_COLUMN_USAGE
             WHERE TABLE_SCHEMA = '{}' 
               AND TABLE_NAME = '{}'
               AND CONSTRAINT_NAME = 'PRIMARY'
             ORDER BY ORDINAL_POSITION",
            database.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| McpError::QueryExecutionError(e.to_string()))?;

        if rows.is_empty() {
            Ok(None)
        } else {
            use sqlx::Row;
            let columns: Vec<String> = rows
                .iter()
                .map(|row| row.try_get("COLUMN_NAME").unwrap_or_default())
                .collect();
            Ok(Some(columns))
        }
    }

    /// Get foreign key information for a table
    async fn get_foreign_keys(
        &self,
        pool: &sqlx::Pool<sqlx::MySql>,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKey>> {
        let query = format!(
            "SELECT 
                CONSTRAINT_NAME as name,
                COLUMN_NAME as column_name,
                REFERENCED_TABLE_NAME as referenced_table,
                REFERENCED_COLUMN_NAME as referenced_column
             FROM information_schema.KEY_COLUMN_USAGE
             WHERE TABLE_SCHEMA = '{}'
               AND TABLE_NAME = '{}'
               AND REFERENCED_TABLE_NAME IS NOT NULL
             ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION",
            database.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| McpError::QueryExecutionError(e.to_string()))?;

        // Group by constraint name
        let mut fk_map: HashMap<String, ForeignKey> = HashMap::new();

        for row in rows {
            use sqlx::Row;
            let name: String = row.try_get("name").unwrap_or_default();
            let column: String = row.try_get("column_name").unwrap_or_default();
            let ref_table: String = row.try_get("referenced_table").unwrap_or_default();
            let ref_column: String = row.try_get("referenced_column").unwrap_or_default();

            fk_map
                .entry(name.clone())
                .or_insert_with(|| ForeignKey {
                    name: name.clone(),
                    columns: vec![],
                    referenced_table: ref_table,
                    referenced_columns: vec![],
                })
                .columns
                .push(column);

            fk_map.get_mut(&name).unwrap().referenced_columns.push(ref_column);
        }

        Ok(fk_map.into_values().collect())
    }

    /// Get index information for a table
    async fn get_indexes(
        &self,
        pool: &sqlx::Pool<sqlx::MySql>,
        database: &str,
        table: &str,
    ) -> Result<Vec<Index>> {
        let query = format!(
            "SELECT 
                INDEX_NAME as name,
                COLUMN_NAME as column_name,
                NON_UNIQUE as non_unique,
                INDEX_TYPE as index_type
             FROM information_schema.STATISTICS
             WHERE TABLE_SCHEMA = '{}'
               AND TABLE_NAME = '{}'
               AND INDEX_NAME != 'PRIMARY'
             ORDER BY INDEX_NAME, SEQ_IN_INDEX",
            database.replace('\'', "''"),
            table.replace('\'', "''")
        );

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| McpError::QueryExecutionError(e.to_string()))?;

        // Group by index name
        let mut index_map: HashMap<String, Index> = HashMap::new();

        for row in rows {
            use sqlx::Row;
            let name: String = row.try_get("name").unwrap_or_default();
            let column: String = row.try_get("column_name").unwrap_or_default();
            let non_unique: i64 = row.try_get("non_unique").unwrap_or(1);
            let index_type: String = row.try_get("index_type").unwrap_or_default();

            index_map
                .entry(name.clone())
                .or_insert_with(|| Index {
                    name: name.clone(),
                    columns: vec![],
                    unique: non_unique == 0,
                    index_type: index_type.clone(),
                })
                .columns
                .push(column);
        }

        Ok(index_map.into_values().collect())
    }
}

/// Information about a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub row_count: Option<u64>,
    pub size_bytes: Option<u64>,
    pub engine: Option<String>,
}

/// Complete schema information for a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnSchema>,
    pub primary_key: Option<Vec<String>>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<Index>,
}

/// Schema information for a column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

/// Foreign key definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub index_type: String,
}

/// List tool for listing data sources and databases
pub struct ListTool {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    // Cache for database lists with timestamp
    database_cache: Arc<RwLock<HashMap<String, (Vec<DatabaseInfo>, std::time::Instant)>>>,
    cache_duration: std::time::Duration,
}

impl ListTool {
    /// Create a new list tool
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    ) -> Self {
        Self {
            manager,
            pool_managers,
            database_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_duration: std::time::Duration::from_secs(60),
        }
    }

    /// List all data sources (without exposing credentials)
    pub async fn list_datasources(&self) -> Vec<DataSourceInfo> {
        tracing::info!("Listing all data sources");
        
        let sources = self.manager.list_sources().await;
        
        tracing::info!(
            count = sources.len(),
            "Listed data sources successfully"
        );
        
        sources
    }

    /// List all databases for a specific data source
    pub async fn list_databases(&self, datasource_key: &str) -> Result<Vec<DatabaseInfo>> {
        // Validate parameters
        if datasource_key.is_empty() {
            return Err(McpError::InvalidStatement(
                "Data source key is required".to_string(),
            ));
        }

        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Check cache first
        {
            let cache = self.database_cache.read().await;
            if let Some((databases, timestamp)) = cache.get(datasource_key) {
                if timestamp.elapsed() < self.cache_duration {
                    tracing::debug!(
                        datasource_key = %datasource_key,
                        age_secs = timestamp.elapsed().as_secs(),
                        "Returning cached database list"
                    );
                    return Ok(databases.clone());
                }
            }
        }

        tracing::info!(
            datasource_key = %datasource_key,
            "Listing databases"
        );

        // Get or create pool manager for this data source
        let mut pool_managers = self.pool_managers.write().await;
        if !pool_managers.contains_key(datasource_key) {
            let config = self
                .manager
                .get_source(datasource_key)
                .ok_or_else(|| McpError::InvalidDataSourceKey(datasource_key.to_string()))?;
            let pool_manager = ConnectionPoolManager::new((*config).clone()).await?;
            pool_managers.insert(datasource_key.to_string(), pool_manager);
        }

        let pool_manager = pool_managers.get_mut(datasource_key).unwrap();
        
        // Connect to information_schema to list databases
        let pool = pool_manager.get_pool("information_schema").await?;

        // Query to get database information
        let query = "SELECT 
                SCHEMA_NAME as name,
                DEFAULT_CHARACTER_SET_NAME as charset,
                DEFAULT_COLLATION_NAME as collation
             FROM information_schema.SCHEMATA
             WHERE SCHEMA_NAME NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys')
             ORDER BY SCHEMA_NAME";

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(query)
            .fetch_all(pool)
            .await
            .map_err(|e| McpError::QueryExecutionError(e.to_string()))?;

        let mut databases: Vec<DatabaseInfo> = Vec::new();

        for row in rows {
            use sqlx::Row;
            let name: String = row.try_get("name").unwrap_or_default();
            let charset: String = row.try_get("charset").unwrap_or_default();
            let collation: String = row.try_get("collation").unwrap_or_default();

            // Get database size
            let size_query = format!(
                "SELECT SUM(DATA_LENGTH + INDEX_LENGTH) as size_bytes
                 FROM information_schema.TABLES
                 WHERE TABLE_SCHEMA = '{}'",
                name.replace('\'', "''")
            );

            let size_row: Option<(Option<i64>,)> = sqlx::query_as(&size_query)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

            let size_bytes = size_row.and_then(|(size,)| size).map(|s| s as u64);

            databases.push(DatabaseInfo {
                name,
                size_bytes,
                charset,
                collation,
            });
        }

        tracing::info!(
            datasource_key = %datasource_key,
            database_count = databases.len(),
            "Listed databases successfully"
        );

        // Update cache
        {
            let mut cache = self.database_cache.write().await;
            cache.insert(
                datasource_key.to_string(),
                (databases.clone(), std::time::Instant::now()),
            );
        }

        Ok(databases)
    }

    /// Clear the database cache for a specific data source
    pub async fn clear_cache(&self, datasource_key: &str) {
        let mut cache = self.database_cache.write().await;
        cache.remove(datasource_key);
        tracing::debug!(
            datasource_key = %datasource_key,
            "Cleared database cache"
        );
    }

    /// Clear all database caches
    pub async fn clear_all_caches(&self) {
        let mut cache = self.database_cache.write().await;
        cache.clear();
        tracing::debug!("Cleared all database caches");
    }
}

/// Information about a database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub size_bytes: Option<u64>,
    pub charset: String,
    pub collation: String,
}

#[cfg(test)]
mod stream_tests {
    use super::*;

    fn create_test_rows(count: usize) -> Vec<QueryRow> {
        (0..count)
            .map(|i| QueryRow {
                values: vec![serde_json::json!(i)],
            })
            .collect()
    }

    fn create_test_columns() -> Vec<ColumnMetadata> {
        vec![ColumnMetadata {
            name: "id".to_string(),
            data_type: "INT".to_string(),
            nullable: false,
        }]
    }

    #[tokio::test]
    async fn test_stream_creation() {
        let columns = create_test_columns();
        let rows = create_test_rows(100);
        let stream = QueryResultStream::new(columns.clone(), rows, 1000);

        assert_eq!(stream.columns().len(), 1);
        assert_eq!(stream.total_rows().await, 100);
        assert_eq!(stream.current_position().await, 0);
        assert!(!stream.is_cancelled().await);
    }

    #[tokio::test]
    async fn test_stream_single_chunk() {
        let columns = create_test_columns();
        let rows = create_test_rows(50);
        let stream = QueryResultStream::new(columns, rows, 1000);

        // First chunk should contain all rows
        let chunk = stream.next_chunk().await.unwrap();
        assert!(chunk.is_some());
        let chunk = chunk.unwrap();
        assert_eq!(chunk.rows.len(), 50);
        assert_eq!(chunk.chunk_number, 0);
        assert!(chunk.is_last);
        assert_eq!(chunk.total_rows, 50);

        // Second call should return None
        let chunk = stream.next_chunk().await.unwrap();
        assert!(chunk.is_none());
    }

    #[tokio::test]
    async fn test_stream_multiple_chunks() {
        let columns = create_test_columns();
        let rows = create_test_rows(2500);
        let stream = QueryResultStream::new(columns, rows, 1000);

        // First chunk: 1000 rows
        let chunk = stream.next_chunk().await.unwrap().unwrap();
        assert_eq!(chunk.rows.len(), 1000);
        assert_eq!(chunk.chunk_number, 0);
        assert!(!chunk.is_last);
        assert_eq!(chunk.total_rows, 2500);

        // Second chunk: 1000 rows
        let chunk = stream.next_chunk().await.unwrap().unwrap();
        assert_eq!(chunk.rows.len(), 1000);
        assert_eq!(chunk.chunk_number, 1);
        assert!(!chunk.is_last);

        // Third chunk: 500 rows (last)
        let chunk = stream.next_chunk().await.unwrap().unwrap();
        assert_eq!(chunk.rows.len(), 500);
        assert_eq!(chunk.chunk_number, 2);
        assert!(chunk.is_last);

        // Fourth call should return None
        let chunk = stream.next_chunk().await.unwrap();
        assert!(chunk.is_none());
    }

    #[tokio::test]
    async fn test_stream_exact_chunk_size() {
        let columns = create_test_columns();
        let rows = create_test_rows(1000);
        let stream = QueryResultStream::new(columns, rows, 1000);

        // Should get exactly one chunk
        let chunk = stream.next_chunk().await.unwrap().unwrap();
        assert_eq!(chunk.rows.len(), 1000);
        assert!(chunk.is_last);

        // Next call should return None
        let chunk = stream.next_chunk().await.unwrap();
        assert!(chunk.is_none());
    }

    #[tokio::test]
    async fn test_stream_cancellation() {
        let columns = create_test_columns();
        let rows = create_test_rows(2000);
        let stream = QueryResultStream::new(columns, rows, 1000);

        // Get first chunk
        let chunk = stream.next_chunk().await.unwrap();
        assert!(chunk.is_some());

        // Cancel the stream
        stream.cancel().await.unwrap();
        assert!(stream.is_cancelled().await);

        // Next chunk should return error
        let result = stream.next_chunk().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamCancelled));
    }

    #[tokio::test]
    async fn test_stream_manager_register() {
        let manager = StreamManager::new();
        let columns = create_test_columns();
        let rows = create_test_rows(100);
        let stream = QueryResultStream::new(columns, rows, 1000);

        let stream_id = manager.register_stream(stream).await;
        assert!(!stream_id.is_empty());
        assert_eq!(manager.active_stream_count().await, 1);
    }

    #[tokio::test]
    async fn test_stream_manager_get_stream() {
        let manager = StreamManager::new();
        let columns = create_test_columns();
        let rows = create_test_rows(100);
        let stream = QueryResultStream::new(columns, rows, 1000);
        let stream_id = manager.register_stream(stream).await;

        let retrieved = manager.get_stream(&stream_id).await;
        assert!(retrieved.is_some());

        let missing = manager.get_stream("nonexistent").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_stream_manager_remove_stream() {
        let manager = StreamManager::new();
        let columns = create_test_columns();
        let rows = create_test_rows(100);
        let stream = QueryResultStream::new(columns, rows, 1000);
        let stream_id = manager.register_stream(stream).await;

        assert_eq!(manager.active_stream_count().await, 1);

        manager.remove_stream(&stream_id).await.unwrap();
        assert_eq!(manager.active_stream_count().await, 0);

        let retrieved = manager.get_stream(&stream_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_stream_manager_concurrent_streams() {
        let manager = StreamManager::new();
        let columns = create_test_columns();

        // Register multiple streams
        let stream1 = QueryResultStream::new(columns.clone(), create_test_rows(100), 1000);
        let stream2 = QueryResultStream::new(columns.clone(), create_test_rows(200), 1000);
        let stream3 = QueryResultStream::new(columns, create_test_rows(300), 1000);

        let id1 = manager.register_stream(stream1).await;
        let id2 = manager.register_stream(stream2).await;
        let id3 = manager.register_stream(stream3).await;

        assert_eq!(manager.active_stream_count().await, 3);

        // Verify each stream is independent
        let s1 = manager.get_stream(&id1).await.unwrap();
        let s2 = manager.get_stream(&id2).await.unwrap();
        let s3 = manager.get_stream(&id3).await.unwrap();

        assert_eq!(s1.total_rows().await, 100);
        assert_eq!(s2.total_rows().await, 200);
        assert_eq!(s3.total_rows().await, 300);

        // Remove one stream
        manager.remove_stream(&id2).await.unwrap();
        assert_eq!(manager.active_stream_count().await, 2);

        // Other streams should still be accessible
        assert!(manager.get_stream(&id1).await.is_some());
        assert!(manager.get_stream(&id2).await.is_none());
        assert!(manager.get_stream(&id3).await.is_some());
    }

    #[tokio::test]
    async fn test_stream_manager_cancel_all() {
        let manager = StreamManager::new();
        let columns = create_test_columns();

        // Register multiple streams
        let stream1 = QueryResultStream::new(columns.clone(), create_test_rows(100), 1000);
        let stream2 = QueryResultStream::new(columns, create_test_rows(200), 1000);

        manager.register_stream(stream1).await;
        manager.register_stream(stream2).await;

        assert_eq!(manager.active_stream_count().await, 2);

        // Cancel all streams
        manager.cancel_all().await.unwrap();
        assert_eq!(manager.active_stream_count().await, 0);
    }

    #[tokio::test]
    async fn test_stream_isolation() {
        let manager = StreamManager::new();
        let columns = create_test_columns();

        // Create two streams
        let stream1 = QueryResultStream::new(columns.clone(), create_test_rows(2000), 1000);
        let stream2 = QueryResultStream::new(columns, create_test_rows(3000), 1000);

        let id1 = manager.register_stream(stream1).await;
        let id2 = manager.register_stream(stream2).await;

        // Get streams
        let s1 = manager.get_stream(&id1).await.unwrap();
        let s2 = manager.get_stream(&id2).await.unwrap();

        // Consume first chunk from stream 1
        let chunk1 = s1.next_chunk().await.unwrap().unwrap();
        assert_eq!(chunk1.rows.len(), 1000);
        assert_eq!(s1.current_position().await, 1000);

        // Stream 2 should be unaffected
        assert_eq!(s2.current_position().await, 0);

        // Consume first chunk from stream 2
        let chunk2 = s2.next_chunk().await.unwrap().unwrap();
        assert_eq!(chunk2.rows.len(), 1000);
        assert_eq!(s2.current_position().await, 1000);

        // Stream 1 position should still be at 1000
        assert_eq!(s1.current_position().await, 1000);

        // Cancel stream 1
        s1.cancel().await.unwrap();
        assert!(s1.is_cancelled().await);

        // Stream 2 should still be active
        assert!(!s2.is_cancelled().await);
        let chunk2_next = s2.next_chunk().await.unwrap();
        assert!(chunk2_next.is_some());
    }
}

/// Stats tool for retrieving connection pool statistics
pub struct StatsTool {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
}

impl StatsTool {
    /// Create a new stats tool
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    ) -> Self {
        Self {
            manager,
            pool_managers,
        }
    }

    /// Get connection statistics for all data sources or a specific data source
    /// If datasource_key is None, returns stats for all data sources
    /// If datasource_key is Some, returns stats for that specific data source
    pub async fn get_connection_stats(
        &self,
        datasource_key: Option<&str>,
    ) -> Result<Vec<ConnectionStats>> {
        if let Some(key) = datasource_key {
            // Get stats for a specific data source
            self.get_single_datasource_stats(key).await
        } else {
            // Get stats for all data sources
            self.get_all_datasource_stats().await
        }
    }

    /// Get connection statistics for a specific data source
    async fn get_single_datasource_stats(&self, datasource_key: &str) -> Result<Vec<ConnectionStats>> {
        // Validate parameters
        if datasource_key.is_empty() {
            return Err(McpError::InvalidStatement(
                "Data source key is required".to_string(),
            ));
        }

        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        tracing::info!(
            datasource_key = %datasource_key,
            "Getting connection statistics for data source"
        );

        let pool_managers = self.pool_managers.read().await;

        // Check if pool manager exists for this data source
        if let Some(pool_manager) = pool_managers.get(datasource_key) {
            // Get stats from the pool manager
            let pool_stats = pool_manager.get_stats();
            
            // Convert to ConnectionStats format
            let stats: Vec<ConnectionStats> = pool_stats
                .into_iter()
                .map(|ps| ConnectionStats {
                    datasource_key: datasource_key.to_string(),
                    database: ps.database,
                    active_connections: ps.active_connections,
                    idle_connections: ps.idle_connections,
                    total_connections: ps.total_connections,
                    queued_requests: 0, // sqlx doesn't expose this directly
                })
                .collect();

            tracing::info!(
                datasource_key = %datasource_key,
                pool_count = stats.len(),
                "Retrieved connection statistics"
            );

            Ok(stats)
        } else {
            // No pool manager exists yet - return empty stats
            tracing::debug!(
                datasource_key = %datasource_key,
                "No connection pools exist yet for this data source"
            );
            Ok(vec![])
        }
    }

    /// Get connection statistics for all data sources
    async fn get_all_datasource_stats(&self) -> Result<Vec<ConnectionStats>> {
        tracing::info!("Getting connection statistics for all data sources");

        let pool_managers = self.pool_managers.read().await;
        let mut all_stats = Vec::new();

        // Iterate through all pool managers
        for (datasource_key, pool_manager) in pool_managers.iter() {
            let pool_stats = pool_manager.get_stats();
            
            // Convert to ConnectionStats format
            for ps in pool_stats {
                all_stats.push(ConnectionStats {
                    datasource_key: datasource_key.clone(),
                    database: ps.database,
                    active_connections: ps.active_connections,
                    idle_connections: ps.idle_connections,
                    total_connections: ps.total_connections,
                    queued_requests: 0, // sqlx doesn't expose this directly
                });
            }
        }

        tracing::info!(
            total_pools = all_stats.len(),
            datasource_count = pool_managers.len(),
            "Retrieved connection statistics for all data sources"
        );

        Ok(all_stats)
    }
}

/// Connection statistics for a specific database pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub datasource_key: String,
    pub database: String,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub total_connections: usize,
    pub queued_requests: usize,
}

#[cfg(test)]
mod stats_tests {
    use super::*;
    use crate::config::{DataSourceConfig, PoolConfig};

    fn create_test_config(key: &str) -> DataSourceConfig {
        DataSourceConfig {
            key: key.to_string(),
            name: "Test Database".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "test".to_string(),
            password: "test".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        }
    }

    #[tokio::test]
    async fn test_stats_tool_validates_empty_datasource_key() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = StatsTool::new(manager, pool_managers);

        let result = tool.get_connection_stats(Some("")).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidStatement(_)));
    }

    #[tokio::test]
    async fn test_stats_tool_validates_invalid_datasource_key() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = StatsTool::new(manager, pool_managers);

        let result = tool.get_connection_stats(Some("invalid-key")).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpError::InvalidDataSourceKey(_)
        ));
    }

    #[tokio::test]
    async fn test_stats_tool_returns_empty_for_no_pools() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = StatsTool::new(manager, pool_managers);

        // Get stats for a valid key but no pools created yet
        let result = tool.get_connection_stats(Some("test-db")).await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.len(), 0);
    }

    #[tokio::test]
    async fn test_stats_tool_returns_all_stats_when_no_key_provided() {
        let configs = vec![create_test_config("test-db")];
        let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));
        let tool = StatsTool::new(manager, pool_managers);

        // Get stats for all data sources (should be empty since no pools created)
        let result = tool.get_connection_stats(None).await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.len(), 0);
    }
}
