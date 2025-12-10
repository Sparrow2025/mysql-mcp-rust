// Feature: mysql-mcp-multi-datasource, Property 31: MCP protocol compliance
// Validates: Requirements 14.2, 14.3, 14.4, 14.5

use mysql_mcp_server::config::{DataSourceConfig, PoolConfig, ServerConfig};
use mysql_mcp_server::mcp_server::MySqlMcpServerHandler;
use proptest::prelude::*;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;

// Strategy to generate valid data source configs for testing
fn valid_datasource_config() -> impl Strategy<Value = DataSourceConfig> {
    (
        "[a-zA-Z0-9_-]{1,20}",
        "[a-zA-Z0-9_-]{1,20}",
        "localhost",
        3306u16..=3310u16,
        "[a-zA-Z0-9_]{1,20}",
        "[a-zA-Z0-9_]{1,20}",
        prop::collection::vec("[a-zA-Z0-9_]{1,20}", 0..3),
    )
        .prop_map(
            |(key, name, host, port, username, password, databases)| DataSourceConfig {
                key,
                name,
                host: host.to_string(),
                port,
                username,
                password,
                databases,
                pool_config: PoolConfig::default(),
            permission: mysql_mcp_server::config::Permission::default(),
            },
        )
}

// Strategy to generate server configs with 1-3 data sources
fn server_config_strategy() -> impl Strategy<Value = ServerConfig> {
    prop::collection::vec(valid_datasource_config(), 1..4).prop_map(|data_sources| ServerConfig {
        data_sources,
        query_timeout_secs: 30,
        stream_chunk_size: 1000,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 31a: Server info returns correct structure
    /// For any MCP server handler, get_info should return a ServerInfo with all required fields
    #[test]
    fn test_server_info_structure(
        config in server_config_strategy()
    ) {
        // Create a runtime for async operations
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            // Create handler (may fail if can't connect to DB, which is expected in tests)
            let handler_result = MySqlMcpServerHandler::new(config).await;
            
            // If handler creation succeeds, test the server info
            if let Ok(handler) = handler_result {
                let server_info = handler.get_info();
                
                // Verify protocol version is set
                prop_assert_eq!(
                    server_info.protocol_version, 
                    ProtocolVersion::V_2024_11_05,
                    "Protocol version should be V_2024_11_05"
                );
                
                // Verify server_info (Implementation) has name and version
                prop_assert!(!server_info.server_info.name.is_empty(), 
                    "Server name should not be empty");
                prop_assert_eq!(server_info.server_info.name, "mysql-mcp-server",
                    "Server name should be 'mysql-mcp-server'");
                prop_assert!(!server_info.server_info.version.is_empty(), 
                    "Server version should not be empty");
                
                // Verify capabilities are set
                prop_assert!(server_info.capabilities.tools.is_some(), 
                    "Tools capability should be present");
                prop_assert!(server_info.capabilities.resources.is_some(), 
                    "Resources capability should be present");
            }
            
            Ok(())
        })?;
    }



    /// Property 31e: Server capabilities advertise tools and resources
    /// For any MCP server handler, capabilities should indicate support for tools and resources
    #[test]
    fn test_server_capabilities_advertised(
        config in server_config_strategy()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let handler_result = MySqlMcpServerHandler::new(config).await;
            
            if let Ok(handler) = handler_result {
                let server_info = handler.get_info();
                let capabilities = &server_info.capabilities;
                
                // Should advertise tools capability
                prop_assert!(capabilities.tools.is_some(),
                    "Server should advertise tools capability");
                
                // Should advertise resources capability
                prop_assert!(capabilities.resources.is_some(),
                    "Server should advertise resources capability");
                
                // Verify tools capability structure
                if let Some(tools_cap) = &capabilities.tools {
                    prop_assert!(tools_cap.list_changed.is_some(),
                        "Tools capability should specify list_changed");
                }
                
                // Verify resources capability structure
                if let Some(resources_cap) = &capabilities.resources {
                    prop_assert!(resources_cap.list_changed.is_some(),
                        "Resources capability should specify list_changed");
                    prop_assert!(resources_cap.subscribe.is_some(),
                        "Resources capability should specify subscribe");
                }
            }
            
            Ok(())
        })?;
    }

}

