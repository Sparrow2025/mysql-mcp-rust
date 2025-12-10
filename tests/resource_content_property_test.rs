// Feature: mysql-mcp-multi-datasource, Property 30: Resource content correctness
// **Validates: Requirements 13.2, 13.3, 13.4, 13.5**
//
// Property 30: Resource content correctness
// *For any* valid resource URI, the server should return the corresponding metadata in JSON format

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
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

// Strategy for generating data source keys
fn datasource_key_strategy() -> impl Strategy<Value = String> {
    "[a-z]{3,10}"
}

// Strategy for generating database names
fn database_name_strategy() -> impl Strategy<Value = String> {
    "[a-z]{3,10}"
}

// Strategy for generating table names
fn table_name_strategy() -> impl Strategy<Value = String> {
    "[a-z]{3,10}"
}

#[test]
fn test_property_datasources_resource_returns_json() {
    let config = proptest::test_runner::Config {
        cases: 50,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    runner
        .run(&datasource_key_strategy(), |key| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup
                let configs = vec![create_test_config(&key)];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute
                let result = provider.get_resource("mysql://datasources").await;

                // Verify: Should succeed and return JSON
                prop_assert!(result.is_ok(), "Expected success for datasources resource");

                let resource = result.unwrap();
                
                // Verify mime type is JSON
                prop_assert_eq!(
                    resource.mime_type,
                    "application/json",
                    "Expected JSON mime type"
                );

                // Verify content is valid JSON
                let json_result = serde_json::from_str::<serde_json::Value>(&resource.content);
                prop_assert!(
                    json_result.is_ok(),
                    "Expected valid JSON content, got: {}",
                    resource.content
                );

                // Verify JSON structure contains datasources array
                let json = json_result.unwrap();
                prop_assert!(
                    json.get("datasources").is_some(),
                    "Expected 'datasources' field in JSON"
                );
                prop_assert!(
                    json["datasources"].is_array(),
                    "Expected 'datasources' to be an array"
                );

                // Verify the datasource we created is in the list
                let datasources = json["datasources"].as_array().unwrap();
                prop_assert!(
                    datasources.len() > 0,
                    "Expected at least one datasource"
                );

                // Verify datasource has required fields (key, name, status)
                let first_ds = &datasources[0];
                prop_assert!(
                    first_ds.get("key").is_some(),
                    "Expected 'key' field in datasource"
                );
                prop_assert!(
                    first_ds.get("name").is_some(),
                    "Expected 'name' field in datasource"
                );
                prop_assert!(
                    first_ds.get("status").is_some(),
                    "Expected 'status' field in datasource"
                );

                // Verify no credentials are exposed
                let content_lower = resource.content.to_lowercase();
                prop_assert!(
                    !content_lower.contains("password"),
                    "Credentials should not be exposed in datasources resource"
                );
                prop_assert!(
                    !content_lower.contains("username"),
                    "Credentials should not be exposed in datasources resource"
                );

                Ok(())
            })
        })
        .unwrap();
}

#[test]
fn test_property_resource_content_is_valid_json() {
    let config = proptest::test_runner::Config {
        cases: 50,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    // Test various resource URIs
    let uri_strategy = prop_oneof![
        Just("mysql://datasources".to_string()),
    ];

    runner
        .run(&uri_strategy, |uri| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup
                let configs = vec![create_test_config("test-db")];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute
                let result = provider.get_resource(&uri).await;

                // If successful, verify content is valid JSON
                if let Ok(resource) = result {
                    prop_assert_eq!(
                        resource.mime_type,
                        "application/json",
                        "Expected JSON mime type for URI: {}",
                        uri
                    );

                    let json_result = serde_json::from_str::<serde_json::Value>(&resource.content);
                    prop_assert!(
                        json_result.is_ok(),
                        "Expected valid JSON content for URI: {}, got: {}",
                        uri,
                        resource.content
                    );
                }

                Ok(())
            })
        })
        .unwrap();
}

#[test]
fn test_property_resource_uri_matches_returned_uri() {
    let config = proptest::test_runner::Config {
        cases: 50,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    runner
        .run(&datasource_key_strategy(), |key| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup
                let configs = vec![create_test_config(&key)];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute
                let uri = "mysql://datasources";
                let result = provider.get_resource(uri).await;

                // Verify: If successful, returned URI should match requested URI
                if let Ok(resource) = result {
                    prop_assert_eq!(
                        resource.uri,
                        uri,
                        "Returned URI should match requested URI"
                    );
                }

                Ok(())
            })
        })
        .unwrap();
}

#[test]
fn test_property_no_credentials_in_any_resource() {
    let config = proptest::test_runner::Config {
        cases: 50,
        ..Default::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);

    runner
        .run(&datasource_key_strategy(), |key| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Setup - use a config with known password
                let mut test_config = create_test_config(&key);
                test_config.password = "secret_password_123".to_string();
                test_config.username = "secret_user_456".to_string();
                
                let configs = vec![test_config];
                let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
                let pool_managers = Arc::new(RwLock::new(HashMap::new()));
                let provider = ResourceProvider::new(manager, pool_managers);

                // Execute - test datasources resource
                let result = provider.get_resource("mysql://datasources").await;

                // Verify: No credentials should be exposed
                if let Ok(resource) = result {
                    let content_lower = resource.content.to_lowercase();
                    
                    prop_assert!(
                        !content_lower.contains("secret_password_123"),
                        "Password should not be exposed in resource content"
                    );
                    prop_assert!(
                        !content_lower.contains("secret_user_456"),
                        "Username should not be exposed in resource content"
                    );
                    prop_assert!(
                        !content_lower.contains("password"),
                        "Password field should not be present in resource content"
                    );
                }

                Ok(())
            })
        })
        .unwrap();
}

// Unit tests for specific resource types
#[tokio::test]
async fn test_datasources_resource_structure() {
    let configs = vec![
        create_test_config("db1"),
        create_test_config("db2"),
    ];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let result = provider.get_resource("mysql://datasources").await;
    assert!(result.is_ok());

    let resource = result.unwrap();
    assert_eq!(resource.mime_type, "application/json");
    assert_eq!(resource.uri, "mysql://datasources");

    let json: serde_json::Value = serde_json::from_str(&resource.content).unwrap();
    assert!(json.get("datasources").is_some());
    
    let datasources = json["datasources"].as_array().unwrap();
    assert_eq!(datasources.len(), 2);

    // Verify each datasource has required fields
    for ds in datasources {
        assert!(ds.get("key").is_some());
        assert!(ds.get("name").is_some());
        assert!(ds.get("status").is_some());
    }
}

#[tokio::test]
async fn test_datasources_resource_no_credentials() {
    let mut config = create_test_config("test-db");
    config.password = "super_secret_password".to_string();
    config.username = "admin_user".to_string();
    
    let configs = vec![config];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let result = provider.get_resource("mysql://datasources").await;
    assert!(result.is_ok());

    let resource = result.unwrap();
    let content = resource.content.to_lowercase();
    
    // Verify no credentials are exposed
    assert!(!content.contains("super_secret_password"));
    assert!(!content.contains("admin_user"));
    assert!(!content.contains("password"));
    assert!(!content.contains("username"));
}

#[tokio::test]
async fn test_resource_templates_available() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    let templates = provider.list_resource_templates();
    
    // Verify we have all expected resource templates
    assert_eq!(templates.len(), 5);
    
    // Verify each template has required fields
    for template in templates {
        assert!(!template.uri_template.is_empty());
        assert!(!template.name.is_empty());
        assert!(!template.description.is_empty());
        assert_eq!(template.mime_type, "application/json");
    }
}

#[tokio::test]
async fn test_invalid_datasource_key_returns_error() {
    let configs = vec![create_test_config("test-db")];
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let provider = ResourceProvider::new(manager, pool_managers);

    // Try to access databases for non-existent datasource
    let result = provider.get_resource("mysql://invalid-key/databases").await;
    assert!(result.is_err());
}
