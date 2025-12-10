// Feature: mysql-mcp-multi-datasource, Property 23: Credential non-disclosure
// Validates: Requirements 10.1, 10.2, 10.3, 10.5

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::error::{sanitize_error_message, McpError};
use mysql_mcp_server::manager::{DataSourceManager, DataSourceInfo};
use proptest::prelude::*;

// Strategy to generate arbitrary passwords (including common patterns)
fn arbitrary_password() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple passwords
        "[a-zA-Z0-9]{8,20}",
        // Passwords with special characters
        "[a-zA-Z0-9!@#$%^&*]{8,20}",
        // Common password patterns
        Just("password123".to_string()),
        Just("admin123".to_string()),
        Just("P@ssw0rd!".to_string()),
        Just("secret".to_string()),
        Just("mypassword".to_string()),
    ]
}

// Strategy to generate arbitrary usernames
fn arbitrary_username() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{4,12}",
        Just("admin".to_string()),
        Just("root".to_string()),
        Just("user".to_string()),
        Just("dbuser".to_string()),
    ]
}

// Strategy to generate arbitrary hosts
fn arbitrary_host() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("localhost".to_string()),
        Just("127.0.0.1".to_string()),
        "[a-z]{5,10}\\.[a-z]{3,5}\\.[a-z]{2,3}",
        "\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}",
    ]
}

// Strategy to generate arbitrary ports
fn arbitrary_port() -> impl Strategy<Value = u16> {
    1u16..=65535u16
}

// Strategy to generate arbitrary data source keys
fn arbitrary_key() -> impl Strategy<Value = String> {
    "[a-z0-9-]{4,20}"
}

// Strategy to generate a valid DataSourceConfig with random credentials
fn datasource_with_credentials() -> impl Strategy<Value = DataSourceConfig> {
    (
        arbitrary_key(),
        "[a-zA-Z ]{5,20}",
        arbitrary_host(),
        arbitrary_port(),
        arbitrary_username(),
        arbitrary_password(),
    )
        .prop_map(|(key, name, host, port, username, password)| DataSourceConfig {
            key,
            name,
            host,
            port,
            username,
            password,
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
        })
}

// Strategy to generate error messages that might contain credentials
fn error_message_with_credentials() -> impl Strategy<Value = (String, String, String)> {
    (arbitrary_username(), arbitrary_password(), arbitrary_host()).prop_flat_map(
        |(username, password, host)| {
            let username_clone = username.clone();
            let password_clone = password.clone();
            let host_clone = host.clone();

            prop_oneof![
                // MySQL connection string format
                Just((
                    format!("mysql://{}:{}@{}", username, password, host),
                    username_clone.clone(),
                    password_clone.clone()
                )),
                // Password field format
                Just((
                    format!("Connection failed: password={}", password),
                    username_clone.clone(),
                    password_clone.clone()
                )),
                // Password with colon
                Just((
                    format!("Error: password:{}", password),
                    username_clone.clone(),
                    password_clone.clone()
                )),
                // pwd field format
                Just((
                    format!("Authentication failed: pwd={}", password),
                    username_clone.clone(),
                    password_clone.clone()
                )),
                // Password in text
                Just((
                    format!("Password: {} is invalid", password),
                    username_clone.clone(),
                    password_clone.clone()
                )),
                // Full connection string with port
                Just((
                    format!("Failed to connect to mysql://{}:{}@{}:3306/db", username, password, host),
                    username_clone.clone(),
                    password_clone.clone()
                )),
                // Generic connection string
                Just((
                    format!("Connection error: {}:{}@{}", username, password, host),
                    username_clone.clone(),
                    password_clone.clone()
                )),
            ]
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 23a: Error message sanitization removes passwords
    /// For any error message containing credentials, sanitization should remove them
    #[test]
    fn test_error_messages_never_contain_passwords(
        (message, _username, password) in error_message_with_credentials()
    ) {
        let sanitized = sanitize_error_message(&message);
        
        // The sanitized message should not contain the password
        prop_assert!(
            !sanitized.contains(&password),
            "Sanitized error message still contains password '{}': '{}'",
            password,
            sanitized
        );
        
        // The sanitized message should contain [REDACTED]
        prop_assert!(
            sanitized.contains("[REDACTED]"),
            "Sanitized error message should contain [REDACTED]: '{}'",
            sanitized
        );
    }

    /// Property 23b: McpError sanitize method removes credentials
    /// For any McpError containing credentials, the sanitize method should remove them
    #[test]
    fn test_mcp_error_sanitize_removes_credentials(
        username in arbitrary_username(),
        password in arbitrary_password(),
        host in arbitrary_host(),
        port in arbitrary_port(),
    ) {
        // Create various error types that might contain credentials
        let connection_string = format!("mysql://{}:{}@{}:{}/db", username, password, host, port);
        
        let errors = vec![
            McpError::ConnectionFailed(connection_string.clone()),
            McpError::NetworkError(format!("Failed to connect: {}", connection_string)),
            McpError::ConfigurationError(format!("Invalid config: password={}", password)),
            McpError::QueryExecutionError(format!("Query failed on {}", connection_string)),
            McpError::PoolError(format!("Pool error: {}", connection_string)),
        ];
        
        for error in errors {
            let sanitized = error.sanitize();
            
            // Should not contain password
            prop_assert!(
                !sanitized.contains(&password),
                "Sanitized error '{}' still contains password '{}'",
                sanitized,
                password
            );
            
            // Should not contain username in connection string format
            // (username alone might be acceptable, but not in user:pass@host format)
            let credential_pattern = format!("{}:{}@", username, password);
            prop_assert!(
                !sanitized.contains(&credential_pattern),
                "Sanitized error still contains credential pattern: '{}'",
                sanitized
            );
        }
    }

    /// Property 23c: DataSourceInfo never exposes credentials
    /// For any data source configuration, the DataSourceInfo should never contain credentials
    #[test]
    fn test_datasource_info_never_contains_credentials(
        ds in datasource_with_credentials()
    ) {
        let password = ds.password.clone();
        let username = ds.username.clone();
        
        // Create a DataSourceInfo (simulating what list_sources returns)
        let info = DataSourceInfo {
            key: ds.key.clone(),
            name: ds.name.clone(),
            status: mysql_mcp_server::manager::ConnectionStatus::Available,
        };
        
        // Convert to string representation for checking
        let info_debug = format!("{:?}", info);
        
        // Should not contain password
        prop_assert!(
            !info_debug.contains(&password),
            "DataSourceInfo debug output contains password: '{}'",
            info_debug
        );
        
        // Should not contain username (since it's a credential)
        prop_assert!(
            !info_debug.contains(&username),
            "DataSourceInfo debug output contains username: '{}'",
            info_debug
        );
        
        // Should contain key and name (non-sensitive info)
        prop_assert!(
            info_debug.contains(&info.key),
            "DataSourceInfo should contain key"
        );
        prop_assert!(
            info_debug.contains(&info.name),
            "DataSourceInfo should contain name"
        );
    }

    /// Property 23d: Manager list_sources never exposes credentials
    /// For any set of data source configurations, listing them should never expose credentials
    #[test]
    fn test_manager_list_sources_never_exposes_credentials(
        configs in prop::collection::vec(datasource_with_credentials(), 1..5)
    ) {
        // Collect all passwords and usernames
        let passwords: Vec<String> = configs.iter().map(|c| c.password.clone()).collect();
        let usernames: Vec<String> = configs.iter().map(|c| c.username.clone()).collect();
        
        // Create manager (this is async, so we need tokio runtime)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let manager = rt.block_on(async {
            DataSourceManager::new(configs).await
        });
        
        if let Ok(manager) = manager {
            let sources = rt.block_on(async {
                manager.list_sources().await
            });
            
            // Convert all sources to string for checking
            let sources_str = format!("{:?}", sources);
            
            // Check that no password appears in the output
            for password in &passwords {
                prop_assert!(
                    !sources_str.contains(password),
                    "list_sources output contains password '{}': '{}'",
                    password,
                    sources_str
                );
            }
            
            // Check that no username appears in the output
            for username in &usernames {
                prop_assert!(
                    !sources_str.contains(username),
                    "list_sources output contains username '{}': '{}'",
                    username,
                    sources_str
                );
            }
        }
    }

    /// Property 23e: Connection string patterns are always sanitized
    /// For any connection string pattern, sanitization should remove credentials
    #[test]
    fn test_connection_string_patterns_sanitized(
        username in arbitrary_username(),
        password in arbitrary_password(),
        host in arbitrary_host(),
        port in arbitrary_port(),
        database in "[a-z]{4,10}",
    ) {
        let patterns = vec![
            format!("mysql://{}:{}@{}", username, password, host),
            format!("mysql://{}:{}@{}:{}", username, password, host, port),
            format!("mysql://{}:{}@{}:{}/{}", username, password, host, port, database),
            format!("://{}:{}@{}", username, password, host),
            format!("{}:{}@{}:{}", username, password, host, port),
        ];
        
        for pattern in patterns {
            let sanitized = sanitize_error_message(&pattern);
            
            // Should not contain the original pattern
            prop_assert!(
                !sanitized.contains(&format!("{}:{}", username, password)),
                "Sanitized message still contains username:password pattern: '{}'",
                sanitized
            );
            
            // Should not contain password
            prop_assert!(
                !sanitized.contains(&password),
                "Sanitized message still contains password: '{}'",
                sanitized
            );
            
            // Should contain redaction marker
            prop_assert!(
                sanitized.contains("[REDACTED]"),
                "Sanitized message should contain [REDACTED]: '{}'",
                sanitized
            );
        }
    }

    /// Property 23f: Multiple credentials in same message are all sanitized
    /// For any error message containing multiple credentials, all should be sanitized
    #[test]
    fn test_multiple_credentials_all_sanitized(
        user1 in arbitrary_username(),
        pass1 in arbitrary_password(),
        host1 in arbitrary_host(),
        user2 in arbitrary_username(),
        pass2 in arbitrary_password(),
        host2 in arbitrary_host(),
    ) {
        let message = format!(
            "Failed to connect: mysql://{}:{}@{} and mysql://{}:{}@{}",
            user1, pass1, host1, user2, pass2, host2
        );
        
        let sanitized = sanitize_error_message(&message);
        
        // Should not contain any password
        prop_assert!(
            !sanitized.contains(&pass1),
            "Sanitized message still contains first password: '{}'",
            sanitized
        );
        prop_assert!(
            !sanitized.contains(&pass2),
            "Sanitized message still contains second password: '{}'",
            sanitized
        );
        
        // Should not contain credential patterns
        prop_assert!(
            !sanitized.contains(&format!("{}:{}", user1, pass1)),
            "Sanitized message still contains first credential pattern"
        );
        prop_assert!(
            !sanitized.contains(&format!("{}:{}", user2, pass2)),
            "Sanitized message still contains second credential pattern"
        );
    }

    /// Property 23g: Password field variations are all sanitized
    /// For any password field format (password=, pwd=, Password:, etc.), sanitization should work
    #[test]
    fn test_password_field_variations_sanitized(
        password in arbitrary_password(),
    ) {
        let variations = vec![
            format!("password={}", password),
            format!("password:{}", password),
            format!("pwd={}", password),
            format!("pwd:{}", password),
            format!("Password: {}", password),
            format!("Pass: {}", password),
            format!("password = {}", password),
        ];
        
        for variation in variations {
            let sanitized = sanitize_error_message(&variation);
            
            prop_assert!(
                !sanitized.contains(&password),
                "Sanitized message '{}' still contains password '{}' from variation '{}'",
                sanitized,
                password,
                variation
            );
        }
    }

    /// Property 23h: Sanitization preserves non-sensitive information
    /// For any error message, sanitization should preserve non-sensitive parts
    #[test]
    fn test_sanitization_preserves_non_sensitive_info(
        password in arbitrary_password(),
        error_type in "[A-Z][a-z]{5,15}",
        context in "[a-z ]{10,30}",
    ) {
        let message = format!(
            "{} error: {} - password={}",
            error_type, context, password
        );
        
        let sanitized = sanitize_error_message(&message);
        
        // Should not contain password
        prop_assert!(
            !sanitized.contains(&password),
            "Sanitized message contains password"
        );
        
        // Should preserve error type and context
        prop_assert!(
            sanitized.contains(&error_type),
            "Sanitized message should preserve error type '{}': '{}'",
            error_type,
            sanitized
        );
        prop_assert!(
            sanitized.contains(&context),
            "Sanitized message should preserve context '{}': '{}'",
            context,
            sanitized
        );
    }
}
