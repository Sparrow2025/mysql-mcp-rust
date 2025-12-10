# Quick Reference Guide

A quick reference for common MySQL MCP Server operations.

## Configuration

### Minimal Configuration

```toml
[[data_sources]]
key = "my-db"
name = "My Database"
host = "localhost"
port = 3306
username = "root"
password = "$MYSQL_PASSWORD"
databases = []
```

### Environment Variable

```bash
export MYSQL_PASSWORD="your_password"
```

### Start Server

```bash
./mysql-mcp-server
```

## Common Tools

### Query Data

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "my-db",
    "database": "mydb",
    "query": "SELECT * FROM users LIMIT 10"
  }
}
```

### Insert Data

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "my-db",
    "database": "mydb",
    "statement": "INSERT INTO users (name, email) VALUES ('John', 'john@example.com')"
  }
}
```

### Update Data

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "my-db",
    "database": "mydb",
    "statement": "UPDATE users SET status = 'active' WHERE id = 1"
  }
}
```

### Delete Data

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "my-db",
    "database": "mydb",
    "statement": "DELETE FROM users WHERE id = 1"
  }
}
```

### List Data Sources

```json
{
  "tool": "mysql_list_datasources",
  "arguments": {}
}
```

### List Databases

```json
{
  "tool": "mysql_list_databases",
  "arguments": {
    "datasource_key": "my-db"
  }
}
```

### List Tables

```json
{
  "tool": "mysql_list_tables",
  "arguments": {
    "datasource_key": "my-db",
    "database": "mydb"
  }
}
```

### Describe Table

```json
{
  "tool": "mysql_describe_table",
  "arguments": {
    "datasource_key": "my-db",
    "database": "mydb",
    "table": "users"
  }
}
```

### Get Connection Stats

```json
{
  "tool": "mysql_get_connection_stats",
  "arguments": {
    "datasource_key": "my-db"
  }
}
```

## Resource URIs

| URI Pattern | Description |
|------------|-------------|
| `mysql://datasources` | List all data sources |
| `mysql://{key}/databases` | List databases |
| `mysql://{key}/{db}/tables` | List tables |
| `mysql://{key}/{db}/tables/{table}` | Get table schema |
| `mysql://{key}/{db}/schema` | Get complete database schema |

## Configuration Options

### Server Settings

| Option | Default | Description |
|--------|---------|-------------|
| `query_timeout_secs` | 30 | Query timeout in seconds |
| `stream_chunk_size` | 1000 | Rows per stream chunk |

### Data Source Settings

| Option | Required | Description |
|--------|----------|-------------|
| `key` | Yes | Unique identifier |
| `name` | Yes | Human-readable name |
| `host` | Yes | MySQL host |
| `port` | Yes | MySQL port |
| `username` | Yes | MySQL username |
| `password` | Yes | MySQL password |
| `databases` | No | Allowed databases (empty = all) |

### Pool Settings

| Option | Default | Description |
|--------|---------|-------------|
| `max_connections` | 10 | Maximum connections |
| `min_connections` | 2 | Minimum connections |
| `connection_timeout_secs` | 30 | Connection timeout |
| `idle_timeout_secs` | 300 | Idle timeout |
| `max_lifetime_secs` | 1800 | Max connection lifetime |

## Error Codes

| Code | Description |
|------|-------------|
| `AUTHENTICATION_ERROR` | Invalid data source key |
| `DATABASE_NOT_FOUND` | Database doesn't exist |
| `CONNECTION_FAILED` | Can't connect to MySQL |
| `QUERY_TIMEOUT` | Query exceeded timeout |
| `QUERY_EXECUTION_ERROR` | SQL error |
| `DDL_NOT_ALLOWED` | DDL statement rejected |
| `CONNECTION_TIMEOUT` | Pool exhausted |

## Command Line

### Start with Default Config

```bash
./mysql-mcp-server
```

### Start with Custom Config

```bash
MCP_CONFIG_PATH=/path/to/config.toml ./mysql-mcp-server
```

### With Environment Variables

```bash
export MYSQL_PASSWORD="password"
export MCP_CONFIG_PATH="config.toml"
./mysql-mcp-server
```

## Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test

```bash
cargo test test_name
```

### Run Property Tests

```bash
cargo test --test '*_property_test'
```

### Run with Logging

```bash
RUST_LOG=debug cargo test
```

## Building

### Debug Build

```bash
cargo build
```

### Release Build

```bash
cargo build --release
```

### Check Without Building

```bash
cargo check
```

## Common Patterns

### Pagination

```sql
SELECT * FROM table ORDER BY id LIMIT 100 OFFSET 0  -- Page 1
SELECT * FROM table ORDER BY id LIMIT 100 OFFSET 100  -- Page 2
```

### Search

```sql
SELECT * FROM table WHERE name LIKE '%search%' LIMIT 20
```

### Aggregation

```sql
SELECT category, COUNT(*) as count FROM table GROUP BY category
```

### Join

```sql
SELECT a.*, b.name FROM table_a a JOIN table_b b ON a.b_id = b.id
```

## Security Checklist

- [ ] Use environment variables for passwords
- [ ] Add config.toml to .gitignore
- [ ] Restrict database access with `databases` array
- [ ] Use strong passwords
- [ ] Configure appropriate connection limits
- [ ] Set reasonable query timeouts
- [ ] Monitor connection pool statistics
- [ ] Review MySQL user permissions
- [ ] Enable MySQL SSL/TLS if needed
- [ ] Regularly rotate passwords

## Performance Tips

1. **Use Indexes**: Ensure frequently queried columns have indexes
2. **Limit Results**: Always use LIMIT clauses
3. **Stream Large Results**: Enable streaming for > 1000 rows
4. **Optimize Pool Size**: Adjust based on workload
5. **Cache When Possible**: Database lists are cached for 60s
6. **Monitor Stats**: Check connection pool statistics regularly
7. **Use Appropriate Timeouts**: Balance between responsiveness and completion

## Monitoring

### Check Connection Stats

```json
{
  "tool": "mysql_get_connection_stats",
  "arguments": {}
}
```

### Watch Logs

The server logs connection pool statistics every 60 seconds:

```
INFO connection_pool_stats datasource=prod-db-01 active=5 idle=3 total=8 queued=0
```

### Health Indicators

- **Active Connections**: Should be < max_connections
- **Queued Requests**: Should be 0 or low
- **Idle Connections**: Should be >= min_connections
- **Error Rate**: Should be low in logs

## Troubleshooting Quick Fixes

| Problem | Quick Fix |
|---------|-----------|
| Can't connect | Check host, port, credentials |
| Env var not found | `export VAR_NAME="value"` |
| Config validation error | Check all required fields |
| Query timeout | Increase `query_timeout_secs` |
| Pool exhausted | Increase `max_connections` |
| Permission denied | Grant MySQL permissions |
| DDL rejected | Use mysql_query for DDL (read-only) |

## Links

- [Full Documentation](../README.md)
- [Configuration Guide](configuration.md)
- [Environment Variables](environment-variables.md)
- [Usage Examples](examples.md)
