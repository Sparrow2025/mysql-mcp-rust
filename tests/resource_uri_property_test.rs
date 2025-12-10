// Feature: mysql-mcp-multi-datasource, Property 29: Resource URI validation
// **Validates: Requirements 13.1**
//
// Property 29: Resource URI validation
// *For any* resource request, if the URI format is invalid, the server should return an error

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::error::McpError;
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::resources::ResourceProvider;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
            permission: mysql_mcp_server::config::Permission::default(),
    }
}

// Strategy for generating invalid URIs
fn invalid_uri_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Missing protocol
        Just("datasources".to_string()),
        Just("test-db/databases".to_string()),
        
        // Wrong protocol
        Just("http://datasources".to_string()),
        Just("https://test-db/databases".to_string()),
        Just("postgres://datasources".to_string()),
        
        // Invalid path structures
        Just("mysql://".to_string()),
        Just("mysql:///".to_string()),
        Just("mysql://invalid".to_string()),
        Just("mysql://test-db".to_string()),
        Just("mysql://test-db/".to_string()),
        Just("mysql://test-db/invalid-path".to_string()),
        Just("mysql://test-db/databases/extra".to_string()),
        Just("mysql://test-db/db/invalid".to_string()),
        Just("mysql://test-db/db/tables/table/extra".to_string()),
        Just("mysql://test-db/db/schema/extra".to_string()),
        
        // Random invalid strings
        "[a-z]{5,20}",
        "mysql://[a-z]{5,15}/[a-z]{5,15}",
        "mysql://[a-z]{5,15}/[a-z]{5,15}/[a-z]{5,15}",
    ]
}

// Strategy for generating valid URI patterns (without actual validation against DB)
fn valid_uri_pattern_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("mysql://datasources".to_string()),
        "[a-z]{3,10}".prop_map(|key| format!("mysql://{}/databases", key)),
        ("[a-z]{3,10}", "[a-z]{3,10}").prop_map(|(key, db)| format!("mysql://{}/{}/tables", key, db)),
        ("[a-z]{3,10}", "[a-z]{3,10}", "[a-z]{3,10}").prop_map(|(key, db, table)| {
            format!("mysql://{}/{}/tables/{}", key, db, table)
        }),
        ("[a-z]{3,10}", "[a-z]{3,10}").prop_map(|(key, db)| format!("mysql://{}/{}/schema", key, db)),
    ]
}

#[test]
fn test_property_invalid_uri_returns_error() {
    let config = proptest::test_runner::Config {
        cases: 100,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    runner
        .run(&invalid_uri_strategy(), |invalid_uri| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup
                let configs = vec![create_test_config("test-db")];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute
                let result = provider.get_resource(&invalid_uri).await;

                // Verify: Invalid URIs should return an error
                prop_assert!(
                    result.is_err(),
                    "Expected error for invalid URI '{}', but got success",
                    invalid_uri
                );

                // Verify it's specifically an InvalidResourceUri error
                if let Err(e) = result {
                    prop_assert!(
                        matches!(e, McpError::InvalidResourceUri(_)),
                        "Expected InvalidResourceUri error for '{}', but got: {:?}",
                        invalid_uri,
                        e
                    );
                }

                Ok(())
            })
        })
        .unwrap();
}

#[test]
fn test_property_valid_uri_pattern_accepted() {
    let config = proptest::test_runner::Config {
        cases: 100,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    runner
        .run(&valid_uri_pattern_strategy(), |valid_uri| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup
                let configs = vec![create_test_config("test-db")];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute
                let result = provider.get_resource(&valid_uri).await;

                // Verify: Valid URI patterns should NOT return InvalidResourceUri error
                // They may fail for other reasons (e.g., data source not found, database not found)
                // but the URI format itself should be accepted
                if let Err(e) = result {
                    prop_assert!(
                        !matches!(e, McpError::InvalidResourceUri(_)),
                        "Valid URI pattern '{}' was rejected as invalid: {:?}",
                        valid_uri,
                        e
                    );
                }

                Ok(())
            })
        })
        .unwrap();
}

#[test]
fn test_property_uri_with_trailing_slashes_handled() {
    let config = proptest::test_runner::Config {
        cases: 50,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    runner
        .run(&"/{0,5}".prop_map(|slashes| format!("mysql://datasources{}", slashes)), |uri_with_slashes| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup
                let configs = vec![create_test_config("test-db")];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute
                let result = provider.get_resource(&uri_with_slashes).await;

                // Verify: URIs with trailing slashes should be handled (not rejected as invalid format)
                if let Err(e) = result {
                    prop_assert!(
                        !matches!(e, McpError::InvalidResourceUri(_)),
                        "URI with trailing slashes '{}' was rejected as invalid: {:?}",
                        uri_with_slashes,
                        e
                    );
                }

                Ok(())
            })
        })
        .unwrap();
}

// Unit tests for specific edge cases
#[tokio::test]
async fn test_empty_uri_returns_error() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let result = provider.get_resource("").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidResourceUri(_)));
}

#[tokio::test]
async fn test_datasources_uri_is_valid() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let result = provider.get_resource("mysql://datasources").await;
    // Should succeed (datasources always exists)
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_databases_uri_pattern_is_valid() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    // Valid pattern, but data source exists
    let result = provider.get_resource("mysql://test-db/databases").await;
    // May fail due to connection issues, but URI format should be valid
    if let Err(e) = result {
        assert!(!matches!(e, McpError::InvalidResourceUri(_)));
    }
}

#[tokio::test]
async fn test_invalid_protocol_returns_error() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let result = provider.get_resource("http://datasources").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidResourceUri(_)));
}

#[tokio::test]
async fn test_missing_protocol_returns_error() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let result = provider.get_resource("datasources").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidResourceUri(_)));
}
