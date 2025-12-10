// Feature: mysql-mcp-multi-datasource, Property 26: DDL statement rejection
// Validates: Requirements 11.3

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

// Strategy to generate DDL statements that should be rejected
fn arbitrary_ddl_statement() -> impl Strategy<Value = String> {
    prop_oneof![
        // CREATE statements
        Just("CREATE TABLE users (id INT PRIMARY KEY)".to_string()),
        Just("CREATE INDEX idx_name ON users(name)".to_string()),
        Just("CREATE DATABASE testdb".to_string()),
        // ALTER statements
        Just("ALTER TABLE users ADD COLUMN email VARCHAR(255)".to_string()),
        Just("ALTER TABLE users DROP COLUMN name".to_string()),
        Just("ALTER TABLE users MODIFY COLUMN id BIGINT".to_string()),
        // DROP statements
        Just("DROP TABLE users".to_string()),
        Just("DROP INDEX idx_name ON users".to_string()),
        Just("DROP DATABASE testdb".to_string()),
        // TRUNCATE statements
        Just("TRUNCATE TABLE users".to_string()),
        Just("TRUNCATE users".to_string()),
        // RENAME statements
        Just("RENAME TABLE users TO customers".to_string()),
        Just("RENAME TABLE old_name TO new_name".to_string()),
    ]
}

// Strategy to generate DDL statements with different casing
fn arbitrary_ddl_statement_with_casing() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("create table users (id int)".to_string()),
        Just("CREATE TABLE users (id int)".to_string()),
        Just("CrEaTe TaBlE users (id int)".to_string()),
        Just("alter table users add column name varchar(255)".to_string()),
        Just("ALTER TABLE users ADD COLUMN name VARCHAR(255)".to_string()),
        Just("drop table users".to_string()),
        Just("DROP TABLE users".to_string()),
        Just("truncate table users".to_string()),
        Just("TRUNCATE TABLE users".to_string()),
        Just("rename table users to customers".to_string()),
        Just("RENAME TABLE users TO customers".to_string()),
    ]
}

// Strategy to generate DDL statements with leading whitespace
fn arbitrary_ddl_statement_with_whitespace() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("  CREATE TABLE users (id INT)".to_string()),
        Just("\t\tCREATE TABLE users (id INT)".to_string()),
        Just("\n\nCREATE TABLE users (id INT)".to_string()),
        Just("  ALTER TABLE users ADD COLUMN name VARCHAR(255)".to_string()),
        Just("\t\tDROP TABLE users".to_string()),
        Just("  TRUNCATE TABLE users".to_string()),
        Just("\n\nRENAME TABLE users TO customers".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    /// Property 26: DDL statement rejection
    /// For any DDL statement (CREATE, ALTER, DROP, TRUNCATE, RENAME), the execute tool
    /// should reject it and return an error
    #[test]
    fn test_ddl_statements_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        ddl_statement in arbitrary_ddl_statement(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            // Execute the DDL statement
            let result = tool.execute(&datasource_key, &database, &ddl_statement).await;

            // DDL statements should always be rejected
            prop_assert!(result.is_err(),
                "DDL statement '{}' should be rejected", ddl_statement);
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                    "DDL statement '{}' should return DdlNotAllowed error, but got: {:?}",
                    ddl_statement, e
                );
            }
            
            Ok(())
        });
    }

    /// Property 26a: DDL statements rejected regardless of casing
    /// For any DDL statement with any casing (lowercase, uppercase, mixed), it should be rejected
    #[test]
    fn test_ddl_statements_rejected_any_casing(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        ddl_statement in arbitrary_ddl_statement_with_casing(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, &ddl_statement).await;

            prop_assert!(result.is_err(),
                "DDL statement '{}' should be rejected regardless of casing", ddl_statement);
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                    "DDL statement '{}' should return DdlNotAllowed error, but got: {:?}",
                    ddl_statement, e
                );
            }
            
            Ok(())
        });
    }

    /// Property 26b: DDL statements rejected with leading whitespace
    /// For any DDL statement with leading whitespace, it should still be rejected
    #[test]
    fn test_ddl_statements_rejected_with_whitespace(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
        ddl_statement in arbitrary_ddl_statement_with_whitespace(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let _ = rt.block_on(async {
            let datasource_key = config.key.clone();
            let configs = vec![config];
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let tool = ExecuteTool::new(manager, pool_managers);

            let result = tool.execute(&datasource_key, &database, &ddl_statement).await;

            prop_assert!(result.is_err(),
                "DDL statement '{}' should be rejected even with leading whitespace", ddl_statement);
            
            if let Err(e) = result {
                prop_assert!(
                    matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                    "DDL statement '{}' should return DdlNotAllowed error, but got: {:?}",
                    ddl_statement, e
                );
            }
            
            Ok(())
        });
    }

    /// Property 26c: CREATE statements always rejected
    /// For any statement starting with CREATE, it should be rejected
    #[test]
    fn test_create_statements_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let create_statements = vec![
            "CREATE TABLE test (id INT)",
            "CREATE INDEX idx ON test(id)",
            "CREATE DATABASE db",
            "CREATE VIEW v AS SELECT * FROM test",
        ];

        for statement in create_statements {
            let _ = rt.block_on(async {
                let datasource_key = config.key.clone();
                let configs = vec![config.clone()];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = ExecuteTool::new(manager, pool_managers);

                let result = tool.execute(&datasource_key, &database, statement).await;

                prop_assert!(result.is_err(),
                    "CREATE statement '{}' should be rejected", statement);
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                        "CREATE statement should return DdlNotAllowed error, but got: {:?}", e
                    );
                }
                
                Ok(())
            });
        }
    }

    /// Property 26d: ALTER statements always rejected
    /// For any statement starting with ALTER, it should be rejected
    #[test]
    fn test_alter_statements_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let alter_statements = vec![
            "ALTER TABLE test ADD COLUMN name VARCHAR(255)",
            "ALTER TABLE test DROP COLUMN name",
            "ALTER TABLE test MODIFY COLUMN id BIGINT",
        ];

        for statement in alter_statements {
            let _ = rt.block_on(async {
                let datasource_key = config.key.clone();
                let configs = vec![config.clone()];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = ExecuteTool::new(manager, pool_managers);

                let result = tool.execute(&datasource_key, &database, statement).await;

                prop_assert!(result.is_err(),
                    "ALTER statement '{}' should be rejected", statement);
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                        "ALTER statement should return DdlNotAllowed error, but got: {:?}", e
                    );
                }
                
                Ok(())
            });
        }
    }

    /// Property 26e: DROP statements always rejected
    /// For any statement starting with DROP, it should be rejected
    #[test]
    fn test_drop_statements_rejected(
        config in valid_datasource_config(),
        database in arbitrary_non_empty_string(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        let drop_statements = vec![
            "DROP TABLE test",
            "DROP INDEX idx ON test",
            "DROP DATABASE db",
        ];

        for statement in drop_statements {
            let _ = rt.block_on(async {
                let datasource_key = config.key.clone();
                let configs = vec![config.clone()];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let tool = ExecuteTool::new(manager, pool_managers);

                let result = tool.execute(&datasource_key, &database, statement).await;

                prop_assert!(result.is_err(),
                    "DROP statement '{}' should be rejected", statement);
                
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, mysql_mcp_server::error::McpError::DdlNotAllowed),
                        "DROP statement should return DdlNotAllowed error, but got: {:?}", e
                    );
                }
                
                Ok(())
            });
        }
    }
}
