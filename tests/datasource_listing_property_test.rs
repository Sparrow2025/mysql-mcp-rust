// Feature: mysql-mcp-multi-datasource, Property 14: Data source listing accuracy
// Validates: Requirements 6.1
//
// Property 14: Data source listing accuracy
// For any configured set of data sources, the list-datasources tool should return all data source keys and names

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig};
use mysql_mcp_server::manager::DataSourceManager;
use mysql_mcp_server::tools::ListTool;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

// Generator for valid data source configurations
fn arb_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    (
        "[a-z][a-z0-9-]{2,15}",  // key: lowercase alphanumeric with hyphens
        "[A-Za-z][A-Za-z0-9 ]{2,30}",  // name: alphanumeric with spaces
        "localhost|127\\.0\\.0\\.1",  // host
        3306u16..3320u16,  // port range
        "[a-z][a-z0-9_]{2,15}",  // username
        "[a-zA-Z0-9]{8,20}",  // password
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

// Generator for a list of unique data source configurations
fn arb_datasource_configs() -> impl Strategy<Value = Vec<DataSourceConfig>> {
    proptest::collection::vec(arb_datasource_config(), 1..10)
        .prop_map(|configs| {
            // Ensure unique keys
            let mut seen_keys = HashSet::new();
            let mut unique_configs = Vec::new();
            
            for mut config in configs {
                let mut key = config.key.clone();
                let mut counter = 0;
                
                // Make key unique if needed
                while seen_keys.contains(&key) {
                    counter += 1;
                    key = format!("{}-{}", config.key, counter);
                }
                
                config.key = key.clone();
                seen_keys.insert(key);
                unique_configs.push(config);
            }
            
            unique_configs
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn test_list_datasources_returns_all_configured_sources(configs in arb_datasource_configs()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            // Create manager with the generated configs
            let expected_keys: HashSet<String> = configs.iter().map(|c| c.key.clone()).collect();
            let expected_names: HashMap<String, String> = configs.iter()
                .map(|c| (c.key.clone(), c.name.clone()))
                .collect();
            
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let list_tool = ListTool::new(manager, pool_managers);
            
            // List all data sources
            let sources = list_tool.list_datasources().await;
            
            // Property: All configured data sources should be returned
            let returned_keys: HashSet<String> = sources.iter().map(|s| s.key.clone()).collect();
            let expected_count = expected_keys.len();
            prop_assert_eq!(returned_keys, expected_keys, "All configured data source keys should be returned");
            
            // Property: Each data source should have the correct name
            for source in &sources {
                let expected_name = expected_names.get(&source.key).unwrap();
                prop_assert_eq!(&source.name, expected_name, 
                    "Data source '{}' should have name '{}'", source.key, expected_name);
            }
            
            // Property: The number of returned sources should match the number of configured sources
            prop_assert_eq!(sources.len(), expected_count, 
                "Number of returned sources should match number of configured sources");
            
            Ok::<(), TestCaseError>(())
        })?;
    }
    
    #[test]
    fn test_list_datasources_does_not_expose_credentials(configs in arb_datasource_configs()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(DataSourceManager::new(configs.clone()).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let list_tool = ListTool::new(manager, pool_managers);
            
            // List all data sources
            let sources = list_tool.list_datasources().await;
            
            // Property: No credentials should be exposed in the returned data
            // DataSourceInfo only contains key, name, and status - no password or username
            // This is verified by the type system, but we can also check that the
            // serialized output doesn't contain sensitive information
            let serialized = serde_json::to_string(&sources).unwrap();
            
            for config in &configs {
                prop_assert!(
                    !serialized.contains(&config.password),
                    "Password should not be exposed in data source listing"
                );
                prop_assert!(
                    !serialized.contains(&config.username),
                    "Username should not be exposed in data source listing"
                );
            }
            
            Ok::<(), TestCaseError>(())
        })?;
    }
    
    #[test]
    fn test_list_datasources_includes_status(configs in arb_datasource_configs()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
            let pool_managers = Arc::new(RwLock::new(HashMap::new()));
            let list_tool = ListTool::new(manager, pool_managers);
            
            // List all data sources
            let sources = list_tool.list_datasources().await;
            
            // Property: Each data source should have a status field
            for source in &sources {
                // Status should be either Available or Unavailable
                // We can't check the exact value without a real connection,
                // but we can verify the field exists and is accessible
                let _status = source.status;
            }
            
            Ok::<(), TestCaseError>(())
        })?;
    }
}

#[tokio::test]
async fn test_list_datasources_empty_config() {
    // Edge case: empty configuration is allowed at manager level
    // (validation happens at ServerConfig level)
    let configs = vec![];
    
    let manager = Arc::new(DataSourceManager::new(configs).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    let sources = list_tool.list_datasources().await;
    assert_eq!(sources.len(), 0, "Empty configuration should return empty list");
}

#[tokio::test]
async fn test_list_datasources_single_source() {
    let config = DataSourceConfig {
        key: "test-db".to_string(),
        name: "Test Database".to_string(),
        host: "localhost".to_string(),
        port: 3306,
        username: "test".to_string(),
        password: "secret".to_string(),
        databases: vec![],
        pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
    };
    
    let manager = Arc::new(DataSourceManager::new(vec![config.clone()]).await.unwrap());
    let pool_managers = Arc::new(RwLock::new(HashMap::new()));
    let list_tool = ListTool::new(manager, pool_managers);
    
    let sources = list_tool.list_datasources().await;
    
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].key, "test-db");
    assert_eq!(sources[0].name, "Test Database");
}
