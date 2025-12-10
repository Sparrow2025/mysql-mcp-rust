# Design Document

## Overview

本设计文档描述了一个使用 Rust 实现的 MySQL MCP Server，支持多数据源和多数据库访问。系统采用基于密钥的安全访问机制，确保数据库凭证不会暴露给 Agent。系统使用连接池管理数据库连接，支持流式结果传输以处理大型数据集。

核心设计原则：
- **安全第一**：凭证与访问密钥分离，凭证仅在服务器内部使用
- **高性能**：使用连接池和异步 I/O 提高并发性能
- **可扩展**：支持多数据源和多数据库的灵活配置
- **容错性**：完善的错误处理和自动重连机制

## Architecture

系统采用分层架构：

```
┌─────────────────────────────────────────┐
│           MCP Protocol Layer            │
│  (处理 MCP 请求/响应和流式传输)          │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│          Tool Handler Layer             │
│  (query, schema, list-datasources等)    │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       Data Source Manager Layer         │
│  (管理数据源密钥映射和路由)              │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│      Connection Pool Manager Layer      │
│  (管理每个数据源的连接池)                │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│          MySQL Driver Layer             │
│  (使用 sqlx 或 mysql_async)             │
└─────────────────────────────────────────┘
```

### 关键组件交互流程

1. Agent 通过 MCP 协议发送请求（包含数据源 key）
2. MCP Protocol Layer 解析请求并路由到相应的 Tool Handler
3. Tool Handler 验证数据源 key 并获取对应的数据源配置
4. Connection Pool Manager 从连接池获取连接
5. 执行 SQL 操作并通过流式方式返回结果
6. 连接归还到连接池

## Components and Interfaces

### 1. Configuration Module

负责加载和管理配置信息。

```rust
pub struct DataSourceConfig {
    pub key: String,              // 数据源密钥
    pub name: String,             // 数据源描述名称
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,         // 仅在内部使用，不暴露
    pub databases: Vec<String>,   // 允许访问的数据库列表，空表示全部
    pub pool_config: PoolConfig,
}

pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

pub struct ServerConfig {
    pub data_sources: Vec<DataSourceConfig>,
    pub query_timeout: Duration,
    pub stream_chunk_size: usize,
}
```

### 2. Data Source Manager

管理数据源密钥到配置的映射。

```rust
pub struct DataSourceManager {
    sources: HashMap<String, DataSourceConfig>,  // key -> config
    pools: HashMap<String, ConnectionPoolManager>,
}

impl DataSourceManager {
    pub async fn new(config: ServerConfig) -> Result<Self>;
    pub fn get_source(&self, key: &str) -> Option<&DataSourceConfig>;
    pub async fn get_connection(&self, key: &str, database: &str) -> Result<Connection>;
    pub fn list_sources(&self) -> Vec<DataSourceInfo>;  // 不包含凭证
    pub async fn list_databases(&self, key: &str) -> Result<Vec<DatabaseInfo>>;
}

pub struct DataSourceInfo {
    pub key: String,
    pub name: String,
    pub status: ConnectionStatus,
}
```

### 3. Connection Pool Manager

为每个数据源维护连接池。

```rust
pub struct ConnectionPoolManager {
    pools: HashMap<String, Pool<MySqlConnection>>,  // database -> pool
    config: DataSourceConfig,
}

impl ConnectionPoolManager {
    pub async fn new(config: DataSourceConfig) -> Result<Self>;
    pub async fn get_connection(&self, database: &str) -> Result<PooledConnection>;
    pub async fn health_check(&self) -> Result<()>;
    pub fn get_stats(&self) -> PoolStats;
}

pub struct PoolStats {
    pub active_connections: usize,
    pub idle_connections: usize,
    pub total_connections: usize,
}
```

### 4. MCP Tools (对外提供的功能)

MCP Server 对外提供以下工具，Agent 可以通过 MCP 协议调用这些工具：

#### Tool 1: `mysql_query`
执行 SQL 查询并返回结果。

**输入参数：**
- `datasource_key` (string, required): 数据源密钥
- `database` (string, required): 数据库名称
- `query` (string, required): SQL 查询语句
- `stream` (boolean, optional): 是否使用流式返回，默认 false

**输出：**
- 成功时返回查询结果（包含列信息和行数据）
- 失败时返回错误信息

**示例：**
```json
{
  "datasource_key": "prod-db-01",
  "database": "users",
  "query": "SELECT * FROM accounts WHERE status = 'active' LIMIT 10"
}
```

#### Tool 2: `mysql_list_datasources`
列出所有可用的数据源。

**输入参数：**
无

**输出：**
- 数据源列表，每个包含：
  - `key`: 数据源密钥
  - `name`: 数据源描述名称
  - `status`: 连接状态 (available/unavailable)

**示例输出：**
```json
[
  {
    "key": "prod-db-01",
    "name": "Production Database",
    "status": "available"
  },
  {
    "key": "dev-db-01",
    "name": "Development Database",
    "status": "available"
  }
]
```

#### Tool 3: `mysql_list_databases`
列出指定数据源中的所有数据库。

**输入参数：**
- `datasource_key` (string, required): 数据源密钥

**输出：**
- 数据库列表，每个包含：
  - `name`: 数据库名称
  - `size_bytes`: 数据库大小（字节）
  - `charset`: 字符集
  - `collation`: 排序规则

**示例：**
```json
{
  "datasource_key": "prod-db-01"
}
```

#### Tool 4: `mysql_list_tables`
列出指定数据库中的所有表。

**输入参数：**
- `datasource_key` (string, required): 数据源密钥
- `database` (string, required): 数据库名称

**输出：**
- 表列表，每个包含：
  - `name`: 表名
  - `row_count`: 行数（估算）
  - `size_bytes`: 表大小（字节）
  - `engine`: 存储引擎

**示例：**
```json
{
  "datasource_key": "prod-db-01",
  "database": "users"
}
```

#### Tool 5: `mysql_describe_table`
获取表的详细结构信息。

**输入参数：**
- `datasource_key` (string, required): 数据源密钥
- `database` (string, required): 数据库名称
- `table` (string, required): 表名

**输出：**
- 表结构信息，包含：
  - `columns`: 列定义数组
  - `primary_key`: 主键列
  - `foreign_keys`: 外键定义
  - `indexes`: 索引定义

**示例：**
```json
{
  "datasource_key": "prod-db-01",
  "database": "users",
  "table": "accounts"
}
```

#### Tool 6: `mysql_execute`
执行 DML 语句（INSERT, UPDATE, DELETE）。

**输入参数：**
- `datasource_key` (string, required): 数据源密钥
- `database` (string, required): 数据库名称
- `statement` (string, required): SQL 语句

**输出：**
- `affected_rows`: 影响的行数
- `last_insert_id`: 最后插入的 ID（如果适用）

**示例：**
```json
{
  "datasource_key": "prod-db-01",
  "database": "users",
  "statement": "UPDATE accounts SET status = 'inactive' WHERE last_login < '2023-01-01'"
}
```

#### Tool 7: `mysql_get_connection_stats`
获取连接池统计信息。

**输入参数：**
- `datasource_key` (string, optional): 数据源密钥，如果不提供则返回所有数据源的统计

**输出：**
- 连接池统计信息：
  - `datasource_key`: 数据源密钥
  - `active_connections`: 活跃连接数
  - `idle_connections`: 空闲连接数
  - `total_connections`: 总连接数
  - `queued_requests`: 排队的请求数

**示例：**
```json
{
  "datasource_key": "prod-db-01"
}
```

### 4.2 MCP Resources (资源接口)

MCP Resources 提供对数据库元数据的只读访问，使用 URI 方案访问。

#### Resource URI 格式

所有资源使用以下 URI 格式：
```
mysql://{datasource_key}/{database}/{resource_type}[/{resource_name}]
```

#### Resource 1: 数据源列表
**URI:** `mysql://datasources`

**描述:** 返回所有可用数据源的列表

**内容类型:** `application/json`

**示例输出:**
```json
{
  "datasources": [
    {
      "key": "prod-db-01",
      "name": "Production Database",
      "status": "available"
    }
  ]
}
```

#### Resource 2: 数据库列表
**URI:** `mysql://{datasource_key}/databases`

**描述:** 返回指定数据源中的所有数据库

**内容类型:** `application/json`

**示例:** `mysql://prod-db-01/databases`

#### Resource 3: 表列表
**URI:** `mysql://{datasource_key}/{database}/tables`

**描述:** 返回指定数据库中的所有表

**内容类型:** `application/json`

**示例:** `mysql://prod-db-01/users/tables`

#### Resource 4: 表结构
**URI:** `mysql://{datasource_key}/{database}/tables/{table_name}`

**描述:** 返回指定表的完整结构定义

**内容类型:** `application/json`

**示例:** `mysql://prod-db-01/users/tables/accounts`

**示例输出:**
```json
{
  "table_name": "accounts",
  "columns": [
    {
      "name": "id",
      "data_type": "INT",
      "nullable": false,
      "default_value": null
    }
  ],
  "primary_key": ["id"],
  "foreign_keys": [],
  "indexes": []
}
```

#### Resource 5: 数据库 Schema
**URI:** `mysql://{datasource_key}/{database}/schema`

**描述:** 返回整个数据库的完整 schema（所有表的结构）

**内容类型:** `application/json`

**示例:** `mysql://prod-db-01/users/schema`

### 4.1 Tool Handler Implementation

```rust
pub struct QueryTool {
    manager: Arc<DataSourceManager>,
}

impl QueryTool {
    pub async fn execute(
        &self,
        key: &str,
        database: &str,
        query: &str,
        stream: bool,
    ) -> Result<QueryResult>;
}

pub struct ExecuteTool {
    manager: Arc<DataSourceManager>,
}

impl ExecuteTool {
    pub async fn execute(
        &self,
        key: &str,
        database: &str,
        statement: &str,
    ) -> Result<ExecuteResult>;
}

pub struct SchemaTool {
    manager: Arc<DataSourceManager>,
}

impl SchemaTool {
    pub async fn list_tables(&self, key: &str, database: &str) -> Result<Vec<TableInfo>>;
    pub async fn describe_table(&self, key: &str, database: &str, table: &str) -> Result<TableSchema>;
}

pub struct ListTool {
    manager: Arc<DataSourceManager>,
}

impl ListTool {
    pub fn list_datasources(&self) -> Vec<DataSourceInfo>;
    pub async fn list_databases(&self, key: &str) -> Result<Vec<DatabaseInfo>>;
}

pub struct StatsTool {
    manager: Arc<DataSourceManager>,
}

impl StatsTool {
    pub fn get_connection_stats(&self, key: Option<&str>) -> Result<Vec<ConnectionStats>>;
}
```

### 5. Stream Handler

处理流式结果传输。

```rust
pub struct QueryResultStream {
    cursor: RowCursor,
    chunk_size: usize,
}

impl QueryResultStream {
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<Row>>>;
    pub async fn cancel(&mut self) -> Result<()>;
}

pub struct Row {
    pub columns: Vec<String>,
    pub values: Vec<Value>,
}
```

### 6. MCP Protocol Handler

使用官方 `rmcp` 库实现 MCP 协议通信。

```rust
use rmcp::{Server, ServerBuilder, Tool, Resource};
use rmcp::protocol::{ServerCapabilities, Implementation};

pub struct MySqlMcpServer {
    manager: Arc<DataSourceManager>,
}

impl MySqlMcpServer {
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let manager = DataSourceManager::new(config).await?;
        Ok(Self {
            manager: Arc::new(manager),
        })
    }
    
    pub async fn build_server(self) -> Result<Server> {
        let server = ServerBuilder::new("mysql-mcp-server")
            .version(env!("CARGO_PKG_VERSION"))
            .capabilities(ServerCapabilities {
                tools: Some(true),
                resources: Some(true),
                prompts: None,
                logging: None,
            })
            // Register all tools
            .tool(self.create_query_tool())
            .tool(self.create_execute_tool())
            .tool(self.create_list_datasources_tool())
            .tool(self.create_list_databases_tool())
            .tool(self.create_list_tables_tool())
            .tool(self.create_describe_table_tool())
            .tool(self.create_connection_stats_tool())
            // Register resources
            .resource_provider(self.create_resource_provider())
            .build()?;
            
        Ok(server)
    }
    
    fn create_query_tool(&self) -> Tool {
        // Tool implementation using rmcp::Tool
    }
    
    fn create_resource_provider(&self) -> impl ResourceProvider {
        // Resource provider implementation
    }
}

// Resource URIs for accessing database metadata
// mysql://{datasource_key}/{database}/schema
// mysql://{datasource_key}/{database}/tables
// mysql://{datasource_key}/{database}/tables/{table_name}
```

## Data Models

### Query Result

```rust
pub struct QueryResult {
    pub columns: Vec<ColumnMetadata>,
    pub rows: Vec<Row>,
    pub affected_rows: u64,
    pub execution_time: Duration,
}

pub struct ColumnMetadata {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}
```

### Schema Information

```rust
pub struct TableInfo {
    pub name: String,
    pub row_count: Option<u64>,
    pub size_bytes: Option<u64>,
}

pub struct TableSchema {
    pub table_name: String,
    pub columns: Vec<ColumnSchema>,
    pub primary_key: Option<Vec<String>>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<Index>,
}

pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

pub struct ForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}

pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub index_type: String,
}
```

### Error Types

```rust
pub enum McpError {
    InvalidDataSourceKey(String),
    DatabaseNotFound(String),
    ConnectionFailed(String),
    QueryTimeout,
    QueryExecutionError(String),
    AuthenticationError,
    ConfigurationError(String),
    StreamCancelled,
}
```


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Configuration validation completeness
*For any* data source configuration, if any required field (host, port, username, password) is missing, the validation should reject the configuration
**Validates: Requirements 1.2**

### Property 2: Invalid configuration handling
*For any* data source configuration with invalid values, the server should log an error and skip that data source without crashing
**Validates: Requirements 1.3**

### Property 3: Pool creation consistency
*For any* set of valid data source configurations, the number of created connection pools should equal the number of valid configurations
**Validates: Requirements 1.4**

### Property 4: Query parameter validation
*For any* query request, if either the data source key or database name is missing, the request should be rejected
**Validates: Requirements 2.3**

### Property 5: Invalid database error handling
*For any* query with a non-existent database name, the server should return an error indicating the database does not exist
**Validates: Requirements 2.4**

### Property 6: Connection pool isolation
*For any* two different databases within the same data source, they should have separate connection pools that do not share connections
**Validates: Requirements 2.5**

### Property 7: Query execution correctness
*For any* valid query with correct data source key and database name, the query should execute and return results or an error
**Validates: Requirements 3.1**

### Property 8: Result format consistency
*For any* successful query, the result should contain columns metadata and rows in a structured format
**Validates: Requirements 3.2**

### Property 9: Query error reporting
*For any* invalid SQL query, the server should return an error message with failure details
**Validates: Requirements 3.3**

### Property 10: Multi-statement query handling
*For any* query containing multiple SQL statements separated by semicolons, only the first statement should be executed
**Validates: Requirements 3.4**

### Property 11: Table listing completeness
*For any* database, the schema tool should return all tables present in that database
**Validates: Requirements 4.1**

### Property 12: Table schema completeness
*For any* table, the schema information should include column names, data types, constraints, primary keys, foreign keys, and indexes
**Validates: Requirements 4.2, 4.4, 4.5**

### Property 13: Non-existent table error handling
*For any* non-existent table name, the schema tool should return an error message
**Validates: Requirements 4.3**

### Property 14: Data source listing accuracy
*For any* configured set of data sources, the list-datasources tool should return all data source keys and names
**Validates: Requirements 6.1**

### Property 15: Database listing accuracy
*For any* valid data source key, the list-databases tool should return all accessible databases with metadata
**Validates: Requirements 6.2, 6.4**

### Property 16: Invalid data source key handling
*For any* invalid data source key, operations should return an authentication error
**Validates: Requirements 6.3, 8.4**

### Property 17: Network error descriptiveness
*For any* network error during query execution, the error message should be descriptive and not expose credentials
**Validates: Requirements 7.4**

### Property 18: Data source key uniqueness
*For any* set of configured data sources, all generated or assigned keys should be unique
**Validates: Requirements 8.2**

### Property 19: Key-based authentication
*For any* tool invocation, the server should accept only data source keys and reject any requests containing database credentials
**Validates: Requirements 8.3**

### Property 20: Key-to-credentials mapping correctness
*For any* valid data source key, the server should correctly map it to the corresponding data source credentials internally
**Validates: Requirements 8.5**

### Property 21: Stream chunk size limit
*For any* streaming query result, each chunk should contain no more than 1000 rows
**Validates: Requirements 9.2**

### Property 22: Concurrent stream isolation
*For any* two concurrent query streams, they should operate independently without interference
**Validates: Requirements 9.5**

### Property 23: Credential non-disclosure
*For any* server response (including tool results, errors, logs, and data source listings), no database credentials should be present
**Validates: Requirements 10.1, 10.2, 10.3, 10.5**

### Property 24: DML execution correctness
*For any* valid DML statement (INSERT, UPDATE, DELETE), the execute tool should execute it and return the affected row count
**Validates: Requirements 11.1**

### Property 25: INSERT last ID return
*For any* INSERT statement that generates an auto-increment ID, the execute tool should return the last inserted ID
**Validates: Requirements 11.2**

### Property 26: DDL statement rejection
*For any* DDL statement (CREATE, ALTER, DROP), the execute tool should reject it and return an error
**Validates: Requirements 11.3**

### Property 27: Execute error reporting
*For any* failed execute operation, the server should return a descriptive error message
**Validates: Requirements 11.4**

### Property 28: Connection stats completeness
*For any* data source, the connection stats should include active, idle, total connection counts, and queued request count
**Validates: Requirements 12.3, 12.4**

### Property 29: Resource URI validation
*For any* resource request, if the URI format is invalid, the server should return an error
**Validates: Requirements 13.1**

### Property 30: Resource content correctness
*For any* valid resource URI, the server should return the corresponding metadata in JSON format
**Validates: Requirements 13.2, 13.3, 13.4, 13.5**

### Property 31: MCP protocol compliance
*For any* MCP client request (server info, list tools, list resources), the server should respond according to the MCP protocol specification
**Validates: Requirements 14.2, 14.3, 14.4, 14.5**

## Error Handling

### Error Categories

1. **Configuration Errors**
   - Missing required fields
   - Invalid values (e.g., invalid port numbers)
   - File not found or unreadable
   - Action: Log error, skip invalid configuration, continue with valid ones

2. **Authentication Errors**
   - Invalid data source key
   - Action: Return `AuthenticationError` to caller

3. **Connection Errors**
   - Cannot connect to MySQL server
   - Network timeout
   - Action: Retry with exponential backoff (up to 3 times), mark data source as unavailable if all retries fail

4. **Query Errors**
   - SQL syntax errors
   - Permission denied
   - Query timeout (>30 seconds)
   - Action: Return descriptive error message (sanitized), do not crash

5. **Resource Errors**
   - Connection pool exhausted
   - Memory limits exceeded
   - Action: Queue requests or return temporary error

### Error Response Format

All errors returned to Agent follow this structure:

```rust
pub struct ErrorResponse {
    pub error_code: String,
    pub message: String,
    pub details: Option<HashMap<String, String>>,
    // Never include: credentials, connection strings, internal paths
}
```

### Retry and Recovery Strategy

- **Connection failures**: Retry 3 times with exponential backoff (1s, 2s, 4s)
- **Transient errors**: Retry once immediately
- **Permanent errors**: No retry, return error immediately
- **Unavailable data sources**: Attempt reconnection every 60 seconds in background

## Testing Strategy

### Unit Testing

单元测试将覆盖以下关键场景：

1. **Configuration Loading**
   - 测试从文件加载配置
   - 测试从环境变量加载配置
   - 测试配置验证逻辑

2. **Connection Pool Management**
   - 测试连接池创建
   - 测试连接获取和归还
   - 测试连接池在达到最大连接数时的排队行为
   - 测试连接错误时的移除和重建
   - 测试优雅关闭

3. **Error Handling**
   - 测试各种错误场景的错误消息
   - 测试凭证脱敏逻辑
   - 测试超时处理

4. **Stream Handling**
   - 测试流式结果的分块发送
   - 测试流的取消
   - 测试流的资源清理

### Property-Based Testing

属性测试将使用 Rust 的 `proptest` 或 `quickcheck` 库，每个测试至少运行 100 次迭代。

每个正确性属性将实现为一个独立的属性测试，测试代码中必须包含注释标记，格式为：
`// Feature: mysql-mcp-multi-datasource, Property {number}: {property_text}`

关键属性测试包括：

1. **配置验证属性** (Property 1-3)
   - 生成随机配置（包括缺失字段、无效值）
   - 验证验证逻辑和池创建逻辑

2. **查询处理属性** (Property 4-10)
   - 生成随机查询参数
   - 验证参数验证、执行和错误处理

3. **Schema 操作属性** (Property 11-13)
   - 生成随机数据库和表名
   - 验证 schema 信息的完整性

4. **安全属性** (Property 16, 19, 23)
   - 生成各种请求和响应
   - 验证凭证永不泄露

5. **流式传输属性** (Property 21-22)
   - 生成大型结果集
   - 验证分块大小和并发隔离

### Integration Testing

集成测试将使用真实的 MySQL 数据库（可以使用 Docker 容器）：

1. 端到端查询流程测试
2. 多数据源并发访问测试
3. 连接池在高负载下的行为测试
4. 故障恢复测试（模拟网络中断、数据库重启等）

### Test Data Generation

对于属性测试，我们将实现以下生成器：

```rust
// 生成随机配置
fn arbitrary_config() -> impl Strategy<Value = DataSourceConfig>

// 生成随机 SQL 查询
fn arbitrary_query() -> impl Strategy<Value = String>

// 生成随机数据源 key
fn arbitrary_key() -> impl Strategy<Value = String>

// 生成随机数据库名
fn arbitrary_database() -> impl Strategy<Value = String>
```

## Implementation Notes

### Technology Stack

- **Language**: Rust (edition 2021)
- **Async Runtime**: Tokio
- **MySQL Driver**: `sqlx` with MySQL feature
- **MCP Protocol**: `rmcp` (official Rust MCP implementation) - https://docs.rs/rmcp/latest/rmcp/
- **Configuration**: `serde` with `toml` or `yaml` support
- **Logging**: `tracing` with structured logging
- **Testing**: `proptest` for property-based testing, `tokio::test` for async tests
- **Connection Pooling**: `sqlx::Pool` for MySQL connection management

### Security Considerations

1. **Credential Storage**
   - Credentials stored only in `DataSourceConfig` structs
   - Never serialized or logged
   - Use `secrecy` crate for sensitive strings

2. **Input Validation**
   - Validate all inputs from Agent
   - Sanitize SQL queries (prevent SQL injection via parameterized queries where possible)
   - Limit query complexity and execution time

3. **Resource Limits**
   - Maximum connections per pool: configurable, default 10
   - Maximum concurrent streams: configurable, default 100
   - Query timeout: 30 seconds
   - Stream chunk size: 1000 rows

### Performance Considerations

1. **Connection Pooling**
   - Reuse connections to reduce overhead
   - Lazy connection creation
   - Health checks to remove stale connections

2. **Caching**
   - Cache database lists for 60 seconds
   - Cache schema information for 5 minutes
   - Use LRU cache with size limits

3. **Streaming**
   - Use database cursors for large result sets
   - Backpressure handling to prevent memory exhaustion
   - Async streaming to avoid blocking

### Monitoring and Observability

1. **Metrics**
   - Connection pool statistics (active, idle, total)
   - Query execution times
   - Error rates by type
   - Stream counts and throughput

2. **Logging**
   - Structured logs with trace IDs
   - Log levels: ERROR, WARN, INFO, DEBUG, TRACE
   - Sensitive information always masked

3. **Health Checks**
   - Periodic connection health checks
   - Data source availability status
   - Expose health endpoint for monitoring systems
