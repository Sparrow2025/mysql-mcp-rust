# MySQL MCP Server

A high-performance MySQL Model Context Protocol (MCP) server implementation in Rust that supports multiple data sources with secure, key-based access control.

## Features

- **Multi-Data Source Support**: Connect to multiple MySQL servers simultaneously
- **Secure Key-Based Access**: Access databases using secure keys instead of exposing credentials
- **Connection Pooling**: Efficient connection management with configurable pool settings
- **Streaming Support**: Handle large result sets with memory-efficient streaming
- **Comprehensive Tools**: Query execution, schema inspection, database listing, and more
- **MCP Resources**: Browse database metadata through standardized resource URIs
- **Error Recovery**: Automatic retry with exponential backoff and reconnection
- **Monitoring**: Real-time connection pool statistics and structured logging

## Installation

### Prerequisites

- Rust 1.70 or later
- MySQL 5.7 or later (or MariaDB 10.2+)

### Building from Source

```bash
git clone https://github.com/yourusername/mysql-mcp-rust.git
cd mysql-mcp-rust
cargo build --release
```

The compiled binary will be available at `target/release/mysql-mcp-rust`.

## Configuration

### Configuration File

The server uses a TOML configuration file to define data sources and server settings. Create a `config.toml` file based on the provided `config.example.toml`:

```bash
cp config.example.toml config.toml
```

**Important**: Add `config.toml` to your `.gitignore` to avoid committing sensitive credentials:

```bash
echo "config.toml" >> .gitignore
```

### Configuration Structure

#### Global Settings

```toml
# Query timeout in seconds (default: 30)
query_timeout_secs = 30

# Stream chunk size in rows (default: 1000)
stream_chunk_size = 1000
```

#### Data Source Configuration

Each data source requires the following fields:

```toml
[[data_sources]]
key = "prod-db-01"              # Unique identifier (required)
name = "Production Database"    # Human-readable name (required)
host = "localhost"              # MySQL host (required)
port = 3306                     # MySQL port (required)
username = "root"               # MySQL username (required)
password = "password"           # MySQL password (required)
databases = []                  # Allowed databases (empty = all)

[data_sources.pool_config]
max_connections = 10            # Maximum pool connections
min_connections = 2             # Minimum pool connections
connection_timeout_secs = 30    # Connection timeout
idle_timeout_secs = 300         # Idle connection timeout
max_lifetime_secs = 1800        # Maximum connection lifetime
```

### Environment Variables

For security, it's recommended to store passwords in environment variables:

```toml
[[data_sources]]
key = "prod-db-01"
name = "Production Database"
host = "localhost"
port = 3306
username = "root"
password = "$MYSQL_PASSWORD"    # Loads from environment variable
databases = []
```

Set the environment variable before starting the server:

```bash
export MYSQL_PASSWORD="your_secure_password"
./target/release/mysql-mcp-rust
```

For detailed information about environment variables, see [Environment Variables Guide](docs/environment-variables.md).

### Database Access Control

Restrict access to specific databases by listing them in the `databases` array:

```toml
[[data_sources]]
key = "dev-db-01"
name = "Development Database"
host = "localhost"
port = 3306
username = "dev_user"
password = "$DEV_PASSWORD"
databases = ["test_db", "dev_db"]  # Only these databases are accessible
```

Leave the `databases` array empty to allow access to all databases on the server.

## Usage

### Starting the Server

```bash
# Using default config file (config.toml in current directory)
./target/release/mysql-mcp-rust

# Using custom config file via environment variable
MCP_CONFIG_PATH=/path/to/config.toml ./target/release/mysql-mcp-rust
```

The server communicates via stdio using the MCP protocol.

### Quick Start

1. **Create a configuration file:**
   ```bash
   cp config.example.toml config.toml
   ```

2. **Set your database password as an environment variable:**
   ```bash
   export MYSQL_PASSWORD="your_password"
   ```

3. **Edit config.toml** to match your MySQL server settings

4. **Start the server:**
   ```bash
   ./target/release/mysql-mcp-rust
   ```

5. **Test the connection** using an MCP client

### Integrating with MCP Clients

The server can be integrated with any MCP-compatible client. Here's an example configuration for common clients:

#### Claude Desktop

Add to your Claude Desktop configuration (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "mysql": {
      "command": "/path/to/mysql-mcp-rust",
      "env": {
        "MCP_CONFIG_PATH": "/path/to/config.toml",
        "MYSQL_PASSWORD": "your_password"
      }
    }
  }
}
```

#### Generic MCP Client

```javascript
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const transport = new StdioClientTransport({
  command: '/path/to/mysql-mcp-rust',
  env: {
    MCP_CONFIG_PATH: '/path/to/config.toml',
    MYSQL_PASSWORD: 'your_password'
  }
});

const client = new Client({
  name: 'mysql-client',
  version: '1.0.0'
}, {
  capabilities: {}
});

await client.connect(transport);
```

### Available Tools

The server provides the following MCP tools:

#### 1. `mysql_query`

Execute SQL queries and retrieve results.

**Parameters:**
- `datasource_key` (string, required): Data source identifier
- `database` (string, required): Database name
- `query` (string, required): SQL query statement
- `stream` (boolean, optional): Enable streaming for large results

**Example:**
```json
{
  "datasource_key": "prod-db-01",
  "database": "users",
  "query": "SELECT * FROM accounts WHERE status = 'active' LIMIT 10"
}
```

#### 2. `mysql_execute`

Execute DML statements (INSERT, UPDATE, DELETE).

**Parameters:**
- `datasource_key` (string, required): Data source identifier
- `database` (string, required): Database name
- `statement` (string, required): SQL statement

**Returns:**
- `affected_rows`: Number of rows affected
- `last_insert_id`: Last inserted ID (for INSERT statements)

**Example:**
```json
{
  "datasource_key": "prod-db-01",
  "database": "users",
  "statement": "UPDATE accounts SET status = 'inactive' WHERE last_login < '2023-01-01'"
}
```

#### 3. `mysql_list_datasources`

List all configured data sources.

**Parameters:** None

**Returns:** Array of data sources with keys, names, and status.

#### 4. `mysql_list_databases`

List all databases for a data source.

**Parameters:**
- `datasource_key` (string, required): Data source identifier

**Returns:** Array of databases with metadata (size, charset, collation).

#### 5. `mysql_list_tables`

List all tables in a database.

**Parameters:**
- `datasource_key` (string, required): Data source identifier
- `database` (string, required): Database name

**Returns:** Array of tables with metadata (row count, size, engine).

#### 6. `mysql_describe_table`

Get detailed table structure.

**Parameters:**
- `datasource_key` (string, required): Data source identifier
- `database` (string, required): Database name
- `table` (string, required): Table name

**Returns:** Table schema including columns, primary keys, foreign keys, and indexes.

#### 7. `mysql_get_connection_stats`

Get connection pool statistics.

**Parameters:**
- `datasource_key` (string, optional): Specific data source (omit for all)

**Returns:** Connection pool statistics (active, idle, total connections, queued requests).

### MCP Resources

Access database metadata through resource URIs:

- `mysql://datasources` - List all data sources
- `mysql://{key}/databases` - List databases for a data source
- `mysql://{key}/{database}/tables` - List tables in a database
- `mysql://{key}/{database}/tables/{table}` - Get table schema
- `mysql://{key}/{database}/schema` - Get complete database schema

## Security

### Key-Based Access

The server uses a key-based access model where:

1. Data sources are configured with credentials at server startup
2. Clients use data source keys to access databases
3. Actual credentials are never exposed through the MCP protocol
4. All error messages are sanitized to remove credential information

### Best Practices

1. **Use Environment Variables**: Store passwords in environment variables, not in config files
2. **Restrict Database Access**: Use the `databases` array to limit access to specific databases
3. **Connection Limits**: Configure appropriate connection pool limits for your workload
4. **Query Timeouts**: Set reasonable query timeouts to prevent long-running queries
5. **Monitoring**: Regularly check connection pool statistics and logs

## Error Handling

The server implements comprehensive error handling:

- **Connection Failures**: Automatic retry with exponential backoff (up to 3 attempts)
- **Query Timeouts**: Queries exceeding 30 seconds are automatically terminated
- **Invalid Requests**: Clear error messages for invalid parameters or SQL
- **Resource Exhaustion**: Graceful handling of connection pool limits
- **Automatic Recovery**: Background reconnection for unavailable data sources

## Monitoring and Logging

### Structured Logging

The server uses structured logging with the following levels:

- **ERROR**: Critical errors requiring attention
- **WARN**: Warning conditions
- **INFO**: General informational messages
- **DEBUG**: Detailed debugging information
- **TRACE**: Very detailed trace information

### Connection Pool Statistics

Connection pool statistics are logged every 60 seconds and include:

- Active connections
- Idle connections
- Total connections
- Queued requests

Access real-time statistics using the `mysql_get_connection_stats` tool.

## Performance Tuning

### Connection Pool Configuration

Adjust pool settings based on your workload:

```toml
[data_sources.pool_config]
max_connections = 20        # Increase for high concurrency
min_connections = 5         # Keep warm connections ready
connection_timeout_secs = 60    # Increase for slow networks
idle_timeout_secs = 600     # Adjust based on connection cost
max_lifetime_secs = 3600    # Rotate connections periodically
```

### Query Optimization

- Use the `stream` parameter for large result sets
- Adjust `stream_chunk_size` based on memory constraints
- Set appropriate `query_timeout_secs` for your queries

### Caching

The server implements intelligent caching:

- Database lists: 60 seconds
- Schema information: 5 minutes (configurable)

## Troubleshooting

### Connection Issues

**Problem**: Cannot connect to MySQL server

**Solutions**:
- Verify host and port are correct
- Check MySQL server is running and accessible
- Verify username and password
- Check firewall rules
- Review MySQL user permissions

### Environment Variable Not Found

**Problem**: `Environment variable 'VAR_NAME' not found`

**Solution**: Set the environment variable before starting the server:
```bash
export VAR_NAME="value"
```

### Configuration Validation Errors

**Problem**: Configuration validation fails

**Solutions**:
- Ensure all required fields are present (key, name, host, port, username, password)
- Check for duplicate data source keys
- Verify port numbers are valid (> 0)
- Ensure min_connections ≤ max_connections

### Query Timeouts

**Problem**: Queries timing out

**Solutions**:
- Increase `query_timeout_secs` in configuration
- Optimize slow queries
- Add appropriate indexes
- Use streaming for large result sets

### Permission Denied

**Problem**: MySQL user lacks necessary permissions

**Solutions**:
- Grant appropriate permissions to the MySQL user
- For read-only access: `GRANT SELECT ON database.* TO 'user'@'host';`
- For read-write access: `GRANT SELECT, INSERT, UPDATE, DELETE ON database.* TO 'user'@'host';`
- Flush privileges: `FLUSH PRIVILEGES;`

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with logging
RUST_LOG=debug cargo test
```

### Property-Based Tests

The project includes comprehensive property-based tests using `proptest`:

```bash
# Run property tests
cargo test --test '*_property_test'
```

## Project Information

- **Version**: 0.1.0
- **Language**: Rust (Edition 2021)
- **MCP Protocol**: Implemented using `rmcp` library
- **Database Driver**: `sqlx` with MySQL support
- **Async Runtime**: Tokio

## License

[Your License Here]

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

1. Clone the repository
2. Install Rust (1.70 or later)
3. Install MySQL for testing
4. Run tests: `cargo test`
5. Build: `cargo build`

### Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests
cargo test --test '*'

# Run property-based tests
cargo test --test '*_property_test'

# Run with logging
RUST_LOG=debug cargo test
```

## Documentation

- [Quick Reference](docs/quick-reference.md) - Quick reference for common operations
- [Configuration Guide](docs/configuration.md) - Detailed configuration options and examples
- [Environment Variables Guide](docs/environment-variables.md) - Managing passwords and sensitive data
- [Usage Examples](docs/examples.md) - Practical examples of using all tools and features
- [Implementation Summaries](docs/) - Technical implementation details for each component

## Support

For issues and questions:
- GitHub Issues: [Your Repository URL]
- Documentation: [Your Documentation URL]