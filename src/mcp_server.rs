use crate::config::ServerConfig;
use crate::error::{McpError as AppError, Result};
use crate::manager::DataSourceManager;
use crate::monitoring::MonitoringService;
use crate::pool::ConnectionPoolManager;
use crate::resources::ResourceProvider;
use crate::tools::*;
use rmcp::Error as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// MySQL MCP Server Handler implementation
#[derive(Clone)]
pub struct MySqlMcpServerHandler {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    server_info: ServerInfo,
    _monitoring_service: Arc<tokio::sync::Mutex<Option<MonitoringService>>>,
}

impl MySqlMcpServerHandler {
    /// Cleanup all resources (connection pools, monitoring service)
    pub async fn cleanup(&self) {
        tracing::info!("Cleaning up MCP server handler resources...");

        // Stop monitoring service
        {
            let mut monitoring_guard = self._monitoring_service.lock().await;
            if let Some(mut service) = monitoring_guard.take() {
                service.stop();
                tracing::info!("Monitoring service stopped");
            }
        }

        // Close all connection pools
        {
            let pool_managers_guard = self.pool_managers.read().await;
            for (key, pool_manager) in pool_managers_guard.iter() {
                tracing::info!(datasource_key = %key, "Closing connection pools");
                pool_manager.close_all().await;
            }
        }

        tracing::info!("MCP server handler cleanup complete");
    }

    /// Create a new MySQL MCP Server Handler
    pub async fn new(config: ServerConfig) -> Result<Self> {
        // Create data source manager from config
        let manager = DataSourceManager::new(config.data_sources).await?;
        let manager = Arc::new(manager);

        // Create shared pool managers
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));

        // Create server info
        let server_info = ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                resources: Some(ResourcesCapability {
                    subscribe: Some(false),
                    list_changed: Some(false),
                }),
                prompts: None,
                logging: None,
                experimental: None,
            },
            server_info: Implementation {
                name: "mysql-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: None,
        };

        tracing::info!(
            server_name = %server_info.server_info.name,
            version = %server_info.server_info.version,
            "MCP server handler created successfully"
        );

        // Start monitoring service for periodic connection pool statistics logging
        let monitoring_service = MonitoringService::new(
            manager.clone(),
            pool_managers.clone(),
            60, // Log statistics every 60 seconds
        ).start();

        Ok(Self {
            manager,
            pool_managers,
            server_info,
            _monitoring_service: Arc::new(tokio::sync::Mutex::new(Some(monitoring_service))),
        })
    }
}

impl ServerHandler for MySqlMcpServerHandler {
    fn get_info(&self) -> ServerInfo {
        self.server_info.clone()
    }

    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        tracing::debug!("Listing tools");

        let tools = vec![
            Tool::new(
                "mysql_query",
                "Execute a SQL query on a specified database",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "datasource_key": {
                            "type": "string",
                            "description": "The data source key to connect to"
                        },
                        "database": {
                            "type": "string",
                            "description": "The database name to query"
                        },
                        "query": {
                            "type": "string",
                            "description": "The SQL query to execute"
                        }
                    },
                    "required": ["datasource_key", "database", "query"]
                })).unwrap()),
            ),
            Tool::new(
                "mysql_execute",
                "Execute a DML statement (INSERT, UPDATE, DELETE) on a specified database",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "datasource_key": {
                            "type": "string",
                            "description": "The data source key to connect to"
                        },
                        "database": {
                            "type": "string",
                            "description": "The database name to execute on"
                        },
                        "statement": {
                            "type": "string",
                            "description": "The DML statement to execute"
                        }
                    },
                    "required": ["datasource_key", "database", "statement"]
                })).unwrap()),
            ),
            Tool::new(
                "mysql_list_datasources",
                "List all configured data sources",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {}
                })).unwrap()),
            ),
            Tool::new(
                "mysql_list_databases",
                "List all databases for a specified data source",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "datasource_key": {
                            "type": "string",
                            "description": "The data source key to list databases for"
                        }
                    },
                    "required": ["datasource_key"]
                })).unwrap()),
            ),
            Tool::new(
                "mysql_list_tables",
                "List all tables in a specified database",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "datasource_key": {
                            "type": "string",
                            "description": "The data source key to connect to"
                        },
                        "database": {
                            "type": "string",
                            "description": "The database name to list tables from"
                        }
                    },
                    "required": ["datasource_key", "database"]
                })).unwrap()),
            ),
            Tool::new(
                "mysql_describe_table",
                "Get the schema of a specified table",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "datasource_key": {
                            "type": "string",
                            "description": "The data source key to connect to"
                        },
                        "database": {
                            "type": "string",
                            "description": "The database name"
                        },
                        "table": {
                            "type": "string",
                            "description": "The table name to describe"
                        }
                    },
                    "required": ["datasource_key", "database", "table"]
                })).unwrap()),
            ),
            Tool::new(
                "mysql_get_connection_stats",
                "Get connection pool statistics for data sources",
                Arc::new(serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "datasource_key": {
                            "type": "string",
                            "description": "Optional data source key to get stats for. If not provided, returns stats for all data sources"
                        }
                    }
                })).unwrap()),
            ),
        ];

        Ok(ListToolsResult { tools, next_cursor: None })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Generate a trace ID for this tool call
        let trace_id = uuid::Uuid::new_v4();
        
        async move {
            tracing::info!(
                trace_id = %trace_id,
                tool_name = %request.name,
                "Tool call started"
            );

            let result = match request.name.as_ref() {
                "mysql_query" => self.handle_query_tool(request.arguments.map(|a| serde_json::Value::Object(a))).await,
                "mysql_execute" => self.handle_execute_tool(request.arguments.map(|a| serde_json::Value::Object(a))).await,
                "mysql_list_datasources" => self.handle_list_datasources_tool().await,
                "mysql_list_databases" => self.handle_list_databases_tool(request.arguments.map(|a| serde_json::Value::Object(a))).await,
                "mysql_list_tables" => self.handle_list_tables_tool(request.arguments.map(|a| serde_json::Value::Object(a))).await,
                "mysql_describe_table" => self.handle_describe_table_tool(request.arguments.map(|a| serde_json::Value::Object(a))).await,
                "mysql_get_connection_stats" => self.handle_connection_stats_tool(request.arguments.map(|a| serde_json::Value::Object(a))).await,
                _ => {
                    tracing::error!(trace_id = %trace_id, "Unknown tool requested");
                    return Err(McpError::invalid_params(
                        format!("Unknown tool: {}", request.name),
                        None
                    ))
                }
            };

            match &result {
                Ok(_) => tracing::info!(trace_id = %trace_id, "Tool call completed successfully"),
                Err(e) => tracing::error!(trace_id = %trace_id, error = %e.sanitize(), "Tool call failed"),
            }

            result.map_err(|e| McpError::internal_error(e.sanitize(), None))
        }.await
    }

    async fn list_resources(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourcesResult, McpError> {
        tracing::debug!("Listing resources");

        // For now, return empty list as resources are accessed via templates
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, McpError> {
        tracing::debug!("Listing resource templates");

        let resource_provider = ResourceProvider::new(self.manager.clone(), self.pool_managers.clone());
        let templates = resource_provider.list_resource_templates();

        let resource_templates: Vec<ResourceTemplate> = templates
            .into_iter()
            .map(|t| {
                Annotated::new(RawResourceTemplate {
                    uri_template: t.uri_template,
                    name: t.name,
                    description: Some(t.description),
                    mime_type: Some(t.mime_type),
                }, None)
            })
            .collect();

        Ok(ListResourceTemplatesResult {
            resource_templates,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ReadResourceResult, McpError> {
        // Generate a trace ID for this resource read
        let trace_id = uuid::Uuid::new_v4();
        
        async move {
            tracing::info!(
                trace_id = %trace_id,
                uri = %request.uri,
                "Resource read started"
            );

            let resource_provider = ResourceProvider::new(self.manager.clone(), self.pool_managers.clone());
            
            let content = resource_provider
                .get_resource(&request.uri)
                .await
                .map_err(|e| {
                    tracing::error!(trace_id = %trace_id, error = %e.sanitize(), "Resource read failed");
                    McpError::internal_error(e.sanitize(), None)
                })?;

            tracing::info!(trace_id = %trace_id, "Resource read completed successfully");

            Ok(ReadResourceResult {
                contents: vec![ResourceContents::TextResourceContents {
                    uri: content.uri,
                    mime_type: Some(content.mime_type),
                    text: content.content,
                }],
            })
        }.await
    }
}

impl MySqlMcpServerHandler {
    async fn handle_query_tool(&self, args: Option<serde_json::Value>) -> Result<CallToolResult> {
        let args = args.ok_or_else(|| AppError::InvalidStatement("Missing arguments".to_string()))?;
        
        let datasource_key = args["datasource_key"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("datasource_key is required".to_string()))?;
        let database = args["database"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("database is required".to_string()))?;
        let query = args["query"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("query is required".to_string()))?;

        let tool = QueryTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.execute(datasource_key, database, query).await?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }

    async fn handle_execute_tool(&self, args: Option<serde_json::Value>) -> Result<CallToolResult> {
        let args = args.ok_or_else(|| AppError::InvalidStatement("Missing arguments".to_string()))?;
        
        let datasource_key = args["datasource_key"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("datasource_key is required".to_string()))?;
        let database = args["database"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("database is required".to_string()))?;
        let statement = args["statement"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("statement is required".to_string()))?;

        let tool = ExecuteTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.execute(datasource_key, database, statement).await?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }

    async fn handle_list_datasources_tool(&self) -> Result<CallToolResult> {
        let tool = ListTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.list_datasources().await;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }

    async fn handle_list_databases_tool(&self, args: Option<serde_json::Value>) -> Result<CallToolResult> {
        let args = args.ok_or_else(|| AppError::InvalidStatement("Missing arguments".to_string()))?;
        
        let datasource_key = args["datasource_key"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("datasource_key is required".to_string()))?;

        let tool = ListTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.list_databases(datasource_key).await?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }

    async fn handle_list_tables_tool(&self, args: Option<serde_json::Value>) -> Result<CallToolResult> {
        let args = args.ok_or_else(|| AppError::InvalidStatement("Missing arguments".to_string()))?;
        
        let datasource_key = args["datasource_key"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("datasource_key is required".to_string()))?;
        let database = args["database"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("database is required".to_string()))?;

        let tool = SchemaTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.list_tables(datasource_key, database).await?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }

    async fn handle_describe_table_tool(&self, args: Option<serde_json::Value>) -> Result<CallToolResult> {
        let args = args.ok_or_else(|| AppError::InvalidStatement("Missing arguments".to_string()))?;
        
        let datasource_key = args["datasource_key"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("datasource_key is required".to_string()))?;
        let database = args["database"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("database is required".to_string()))?;
        let table = args["table"]
            .as_str()
            .ok_or_else(|| AppError::InvalidStatement("table is required".to_string()))?;

        let tool = SchemaTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.describe_table(datasource_key, database, table).await?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }

    async fn handle_connection_stats_tool(&self, args: Option<serde_json::Value>) -> Result<CallToolResult> {
        let datasource_key = args.as_ref()
            .and_then(|a| a["datasource_key"].as_str());

        let tool = StatsTool::new(self.manager.clone(), self.pool_managers.clone());
        let result = tool.get_connection_stats(datasource_key).await?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| AppError::QueryExecutionError(e.to_string()))?;

        Ok(CallToolResult::success(vec![
            Annotated::new(RawContent::text(text), None)
        ]))
    }
}
