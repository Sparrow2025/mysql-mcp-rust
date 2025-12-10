// Permission control tests
// Tests that different permission levels correctly restrict operations

use mysql_mcp_server::config::{DataSourceConfig, Permission, PoolConfig};
use mysql_mcp_server::error::McpError;
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::{ExecuteTool, QueryTool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn create_config_with_permission(key: &str, permission: Permission) -> DataSourceConfig {
    DataSourceConfig {
        key: key.to_string(),
        name: format!("Test Database - {:?}", permission),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
        permission,
    }
}

#[tokio::test]
async fn test_query_permission_allows_queries() {
    let configs = vec![create_config_with_permission("query-only", Permission::Query)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should allow query operations
    let result = manager.check_query_permission("query-only");
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_query_permission_denies_updates() {
    let configs = vec![create_config_with_permission("query-only", Permission::Query)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should deny update operations
    let result = manager.check_update_permission("query-only");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_query_permission_denies_ddl() {
    let configs = vec![create_config_with_permission("query-only", Permission::Query)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should deny DDL operations
    let result = manager.check_ddl_permission("query-only");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_update_permission_allows_queries() {
    let configs = vec![create_config_with_permission("update-allowed", Permission::Update)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should allow query operations
    let result = manager.check_query_permission("update-allowed");
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_permission_allows_updates() {
    let configs = vec![create_config_with_permission("update-allowed", Permission::Update)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should allow update operations
    let result = manager.check_update_permission("update-allowed");
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_permission_denies_ddl() {
    let configs = vec![create_config_with_permission("update-allowed", Permission::Update)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should deny DDL operations
    let result = manager.check_ddl_permission("update-allowed");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_ddl_permission_allows_all() {
    let configs = vec![create_config_with_permission("ddl-allowed", Permission::Ddl)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should allow query operations
    assert!(manager.check_query_permission("ddl-allowed").is_ok());

    // Should allow update operations
    assert!(manager.check_update_permission("ddl-allowed").is_ok());

    // Should allow DDL operations
    assert!(manager.check_ddl_permission("ddl-allowed").is_ok());
}

#[tokio::test]
async fn test_execute_tool_respects_query_permission() {
    let configs = vec![create_config_with_permission("query-only", Permission::Query)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let tool = ExecuteTool::new(manager, pool_managers);

    // DML statements should be denied
    let result = tool.execute("query-only", "testdb", "INSERT INTO users VALUES (1, 'test')").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

    let result = tool.execute("query-only", "testdb", "UPDATE users SET name = 'test'").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

    let result = tool.execute("query-only", "testdb", "DELETE FROM users").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_execute_tool_respects_update_permission() {
    let configs = vec![create_config_with_permission("update-allowed", Permission::Update)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let tool = ExecuteTool::new(manager, pool_managers);

    // DDL statements should be denied
    let result = tool.execute("update-allowed", "testdb", "CREATE TABLE users (id INT)").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));

    let result = tool.execute("update-allowed", "testdb", "DROP TABLE users").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_get_permission() {
    let configs = vec![
        create_config_with_permission("query-only", Permission::Query),
        create_config_with_permission("update-allowed", Permission::Update),
        create_config_with_permission("ddl-allowed", Permission::Ddl),
    ];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    assert_eq!(manager.get_permission("query-only"), Some(Permission::Query));
    assert_eq!(manager.get_permission("update-allowed"), Some(Permission::Update));
    assert_eq!(manager.get_permission("ddl-allowed"), Some(Permission::Ddl));
    assert_eq!(manager.get_permission("nonexistent"), None);
}

#[tokio::test]
async fn test_permission_check_with_invalid_key() {
    let configs = vec![create_config_with_permission("valid-key", Permission::Query)];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());

    // Should return InvalidDataSourceKey error
    let result = manager.check_query_permission("invalid-key");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidDataSourceKey(_)));

    let result = manager.check_update_permission("invalid-key");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidDataSourceKey(_)));

    let result = manager.check_ddl_permission("invalid-key");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidDataSourceKey(_)));
}
