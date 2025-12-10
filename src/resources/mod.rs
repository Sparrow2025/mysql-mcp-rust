use crate::error::{McpError, Result};
use crate::manager::DataSourceManager;
use crate::pool::ConnectionPoolManager;
use crate::tools::{DatabaseInfo, TableInfo, TableSchema};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Resource provider for MCP Resources interface
/// Provides read-only access to database metadata through URI-based resources
pub struct ResourceProvider {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
}

impl ResourceProvider {
    /// Create a new resource provider
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    ) -> Self {
        Self {
            manager,
            pool_managers,
        }
    }

    /// Get a resource by URI
    /// Supported URIs:
    /// - mysql://datasources - List all data sources
    /// - mysql://{key}/databases - List databases for a data source
    /// - mysql://{key}/{db}/tables - List tables in a database
    /// - mysql://{key}/{db}/tables/{table} - Get table schema
    /// - mysql://{key}/{db}/schema - Get complete database schema
    pub async fn get_resource(&self, uri: &str) -> Result<ResourceContent> {
        tracing::info!(uri = %uri, "Getting resource");

        // Parse the URI
        let parsed = self.parse_uri(uri)?;

        match parsed {
            ParsedUri::Datasources => self.get_datasources_resource().await,
            ParsedUri::Databases { datasource_key } => {
                self.get_databases_resource(&datasource_key).await
            }
            ParsedUri::Tables {
                datasource_key,
                database,
            } => self.get_tables_resource(&datasource_key, &database).await,
            ParsedUri::TableSchema {
                datasource_key,
                database,
                table,
            } => {
                self.get_table_schema_resource(&datasource_key, &database, &table)
                    .await
            }
            ParsedUri::DatabaseSchema {
                datasource_key,
                database,
            } => {
                self.get_database_schema_resource(&datasource_key, &database)
                    .await
            }
        }
    }

    /// Parse a resource URI
    fn parse_uri(&self, uri: &str) -> Result<ParsedUri> {
        // Remove trailing slashes
        let uri = uri.trim_end_matches('/');

        // Check if it starts with mysql://
        if !uri.starts_with("mysql://") {
            return Err(McpError::InvalidResourceUri(format!(
                "URI must start with 'mysql://': {}",
                uri
            )));
        }

        // Remove the protocol prefix
        let path = &uri[8..]; // "mysql://" is 8 characters

        // Split the path into segments
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match segments.as_slice() {
            // mysql://datasources
            ["datasources"] => Ok(ParsedUri::Datasources),

            // mysql://{key}/databases
            [key, "databases"] => Ok(ParsedUri::Databases {
                datasource_key: key.to_string(),
            }),

            // mysql://{key}/{db}/tables
            [key, db, "tables"] => Ok(ParsedUri::Tables {
                datasource_key: key.to_string(),
                database: db.to_string(),
            }),

            // mysql://{key}/{db}/tables/{table}
            [key, db, "tables", table] => Ok(ParsedUri::TableSchema {
                datasource_key: key.to_string(),
                database: db.to_string(),
                table: table.to_string(),
            }),

            // mysql://{key}/{db}/schema
            [key, db, "schema"] => Ok(ParsedUri::DatabaseSchema {
                datasource_key: key.to_string(),
                database: db.to_string(),
            }),

            _ => Err(McpError::InvalidResourceUri(format!(
                "Invalid resource URI format: {}",
                uri
            ))),
        }
    }

    /// Get datasources resource
    async fn get_datasources_resource(&self) -> Result<ResourceContent> {
        let datasources = self.manager.list_sources().await;

        let content = serde_json::to_string_pretty(&serde_json::json!({
            "datasources": datasources
        }))
        .map_err(|e| McpError::QueryExecutionError(format!("Failed to serialize: {}", e)))?;

        Ok(ResourceContent {
            uri: "mysql://datasources".to_string(),
            mime_type: "application/json".to_string(),
            content,
        })
    }

    /// Get databases resource for a data source
    async fn get_databases_resource(&self, datasource_key: &str) -> Result<ResourceContent> {
        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Get or create pool manager
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
        let pool = pool_manager.get_pool("information_schema").await?;

        // Query databases
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

        let content = serde_json::to_string_pretty(&serde_json::json!({
            "datasource_key": datasource_key,
            "databases": databases
        }))
        .map_err(|e| McpError::QueryExecutionError(format!("Failed to serialize: {}", e)))?;

        Ok(ResourceContent {
            uri: format!("mysql://{}/databases", datasource_key),
            mime_type: "application/json".to_string(),
            content,
        })
    }

    /// Get tables resource for a database
    async fn get_tables_resource(
        &self,
        datasource_key: &str,
        database: &str,
    ) -> Result<ResourceContent> {
        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Get or create pool manager
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

        // Query tables
        let query = format!(
            "SELECT 
                TABLE_NAME as name,
                TABLE_ROWS as row_count,
                DATA_LENGTH + INDEX_LENGTH as size_bytes,
                ENGINE as engine
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = '{}'
             ORDER BY TABLE_NAME",
            database.replace('\'', "''")
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

        let content = serde_json::to_string_pretty(&serde_json::json!({
            "datasource_key": datasource_key,
            "database": database,
            "tables": tables
        }))
        .map_err(|e| McpError::QueryExecutionError(format!("Failed to serialize: {}", e)))?;

        Ok(ResourceContent {
            uri: format!("mysql://{}/{}/tables", datasource_key, database),
            mime_type: "application/json".to_string(),
            content,
        })
    }

    /// Get table schema resource
    async fn get_table_schema_resource(
        &self,
        datasource_key: &str,
        database: &str,
        table: &str,
    ) -> Result<ResourceContent> {
        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Get or create pool manager
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

        // Check if table exists
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

        // Get table schema using helper methods
        let columns = get_columns(pool, database, table).await?;
        let primary_key = get_primary_key(pool, database, table).await?;
        let foreign_keys = get_foreign_keys(pool, database, table).await?;
        let indexes = get_indexes(pool, database, table).await?;

        let schema = TableSchema {
            table_name: table.to_string(),
            columns,
            primary_key,
            foreign_keys,
            indexes,
        };

        let content = serde_json::to_string_pretty(&schema)
            .map_err(|e| McpError::QueryExecutionError(format!("Failed to serialize: {}", e)))?;

        Ok(ResourceContent {
            uri: format!("mysql://{}/{}/tables/{}", datasource_key, database, table),
            mime_type: "application/json".to_string(),
            content,
        })
    }

    /// Get complete database schema resource
    async fn get_database_schema_resource(
        &self,
        datasource_key: &str,
        database: &str,
    ) -> Result<ResourceContent> {
        // Validate data source key
        self.manager.validate_key(datasource_key)?;

        // Check if data source is available
        if !self.manager.is_available(datasource_key).await {
            return Err(McpError::DataSourceUnavailable(format!(
                "Data source '{}' is currently unavailable",
                datasource_key
            )));
        }

        // Get or create pool manager
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

        // Get all tables
        let query = format!(
            "SELECT TABLE_NAME as name
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = '{}'
             ORDER BY TABLE_NAME",
            database.replace('\'', "''")
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

        // Get schema for each table
        let mut schemas = Vec::new();
        for row in rows {
            use sqlx::Row;
            let table_name: String = row.try_get("name").unwrap_or_default();

            let columns = get_columns(pool, database, &table_name).await?;
            let primary_key = get_primary_key(pool, database, &table_name).await?;
            let foreign_keys = get_foreign_keys(pool, database, &table_name).await?;
            let indexes = get_indexes(pool, database, &table_name).await?;

            schemas.push(TableSchema {
                table_name,
                columns,
                primary_key,
                foreign_keys,
                indexes,
            });
        }

        let content = serde_json::to_string_pretty(&serde_json::json!({
            "datasource_key": datasource_key,
            "database": database,
            "tables": schemas
        }))
        .map_err(|e| McpError::QueryExecutionError(format!("Failed to serialize: {}", e)))?;

        Ok(ResourceContent {
            uri: format!("mysql://{}/{}/schema", datasource_key, database),
            mime_type: "application/json".to_string(),
            content,
        })
    }

    /// List all available resource URI templates
    pub fn list_resource_templates(&self) -> Vec<ResourceTemplate> {
        vec![
            ResourceTemplate {
                uri_template: "mysql://datasources".to_string(),
                name: "Data Sources".to_string(),
                description: "List all configured data sources".to_string(),
                mime_type: "application/json".to_string(),
            },
            ResourceTemplate {
                uri_template: "mysql://{datasource_key}/databases".to_string(),
                name: "Databases".to_string(),
                description: "List all databases for a data source".to_string(),
                mime_type: "application/json".to_string(),
            },
            ResourceTemplate {
                uri_template: "mysql://{datasource_key}/{database}/tables".to_string(),
                name: "Tables".to_string(),
                description: "List all tables in a database".to_string(),
                mime_type: "application/json".to_string(),
            },
            ResourceTemplate {
                uri_template: "mysql://{datasource_key}/{database}/tables/{table}".to_string(),
                name: "Table Schema".to_string(),
                description: "Get complete schema for a specific table".to_string(),
                mime_type: "application/json".to_string(),
            },
            ResourceTemplate {
                uri_template: "mysql://{datasource_key}/{database}/schema".to_string(),
                name: "Database Schema".to_string(),
                description: "Get complete schema for all tables in a database".to_string(),
                mime_type: "application/json".to_string(),
            },
        ]
    }
}

/// Parsed URI representation
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedUri {
    Datasources,
    Databases {
        datasource_key: String,
    },
    Tables {
        datasource_key: String,
        database: String,
    },
    TableSchema {
        datasource_key: String,
        database: String,
        table: String,
    },
    DatabaseSchema {
        datasource_key: String,
        database: String,
    },
}

/// Resource content returned by the provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: String,
    pub content: String,
}

/// Resource template for listing available resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplate {
    pub uri_template: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

// Helper functions for getting schema information

/// Get column information for a table
async fn get_columns(
    pool: &sqlx::Pool<sqlx::MySql>,
    database: &str,
    table: &str,
) -> Result<Vec<crate::tools::ColumnSchema>> {
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

    let columns: Vec<crate::tools::ColumnSchema> = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            crate::tools::ColumnSchema {
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
    pool: &sqlx::Pool<sqlx::MySql>,
    database: &str,
    table: &str,
) -> Result<Vec<crate::tools::ForeignKey>> {
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
    let mut fk_map: HashMap<String, crate::tools::ForeignKey> = HashMap::new();

    for row in rows {
        use sqlx::Row;
        let name: String = row.try_get("name").unwrap_or_default();
        let column: String = row.try_get("column_name").unwrap_or_default();
        let ref_table: String = row.try_get("referenced_table").unwrap_or_default();
        let ref_column: String = row.try_get("referenced_column").unwrap_or_default();

        fk_map
            .entry(name.clone())
            .or_insert_with(|| crate::tools::ForeignKey {
                name: name.clone(),
                columns: vec![],
                referenced_table: ref_table,
                referenced_columns: vec![],
            })
            .columns
            .push(column);

        fk_map
            .get_mut(&name)
            .unwrap()
            .referenced_columns
            .push(ref_column);
    }

    Ok(fk_map.into_values().collect())
}

/// Get index information for a table
async fn get_indexes(
    pool: &sqlx::Pool<sqlx::MySql>,
    database: &str,
    table: &str,
) -> Result<Vec<crate::tools::Index>> {
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
    let mut index_map: HashMap<String, crate::tools::Index> = HashMap::new();

    for row in rows {
        use sqlx::Row;
        let name: String = row.try_get("name").unwrap_or_default();
        let column: String = row.try_get("column_name").unwrap_or_default();
        let non_unique: i64 = row.try_get("non_unique").unwrap_or(1);
        let index_type: String = row.try_get("index_type").unwrap_or_default();

        index_map
            .entry(name.clone())
            .or_insert_with(|| crate::tools::Index {
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
