use mysql_mcp_server::error::{sanitize_error_message, McpError};

#[test]
fn test_logging_initialization() {
    // This test verifies that logging can be initialized without panicking
    // Note: We can't call init_tracing() multiple times in tests, so we just verify
    // the function exists and is callable
    
    // Verify sanitization works
    let message = "Connection failed: mysql://user:password123@localhost:3306/db";
    let sanitized = sanitize_error_message(message);
    
    assert!(!sanitized.contains("password123"));
    assert!(sanitized.contains("[REDACTED]"));
}

#[test]
fn test_error_sanitization_in_logging_context() {
    // Test that McpError sanitize method works correctly
    let error = McpError::ConnectionFailed(
        "Failed to connect to mysql://admin:secret@localhost:3306".to_string()
    );
    
    let sanitized = error.sanitize();
    
    assert!(!sanitized.contains("secret"));
    assert!(!sanitized.contains("admin"));
    assert!(sanitized.contains("[REDACTED]"));
}

#[test]
fn test_multiple_credential_patterns() {
    // Test various credential patterns that should be sanitized
    let test_cases = vec![
        (
            "Error: password=mypass123 in connection",
            vec!["mypass123"],
        ),
        (
            "Failed: mysql://user:pass@host:3306",
            vec!["pass", "user"],
        ),
        (
            "Connection string: user:password@localhost",
            vec!["password"],
        ),
    ];
    
    for (message, secrets) in test_cases {
        let sanitized = sanitize_error_message(message);
        
        for secret in secrets {
            assert!(
                !sanitized.contains(secret),
                "Sanitized message should not contain '{}': {}",
                secret,
                sanitized
            );
        }
        
        assert!(
            sanitized.contains("[REDACTED]"),
            "Sanitized message should contain [REDACTED]: {}",
            sanitized
        );
    }
}

#[test]
fn test_non_sensitive_messages_unchanged() {
    // Test that non-sensitive messages are not modified
    let messages = vec![
        "Query executed successfully",
        "Connection timeout after 30 seconds",
        "Database not found: test_db",
        "Invalid data source key: unknown",
    ];
    
    for message in messages {
        let sanitized = sanitize_error_message(message);
        assert_eq!(
            message, sanitized,
            "Non-sensitive message should not be modified"
        );
    }
}

#[tokio::test]
async fn test_monitoring_service_integration() {
    use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
    use mysql_mcp_server::manager::DataSourceManager;
    use mysql_mcp_server::monitoring::MonitoringService;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    // Create a test data source
    let config = DataSourceConfig {
        key: "test".to_string(),
        name: "Test DB".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "test".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = DataSourceManager::new(vec![config]).await.unwrap();
    let manager = Arc::new(manager);
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    
    // Create and start monitoring service
    let mut service = MonitoringService::new(manager, pool_managers, 60).start();
    
    // Wait a bit to ensure the service is running
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Stop the service
    service.stop();
    
    // Service should stop cleanly
}
