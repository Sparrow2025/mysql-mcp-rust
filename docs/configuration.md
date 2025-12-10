# Configuration Guide

## Overview

The MySQL MCP Server uses a configuration file to define data sources and server settings. The configuration file can be in TOML or YAML format (TOML is recommended).

## Configuration File Location

By default, the server looks for a configuration file named `config.toml` in the current directory. You can specify a different location using the `MCP_CONFIG_PATH` environment variable:

```bash
MCP_CONFIG_PATH=/path/to/config.toml mysql-mcp-server
```

## Configuration Structure

### Server Settings

```toml
# Query timeout in seconds (default: 30)
query_timeout_secs = 30

# Stream chunk size in rows (default: 1000)
stream_chunk_size = 1000
```

- `query_timeout_secs`: Maximum time in seconds for a query to execute before timing out
- `stream_chunk_size`: Number of rows to send in each chunk when streaming large result sets

### Data Source Configuration

Each data source represents a MySQL server connection. You can configure multiple data sources:

```toml
[[data_sources]]
key = "prod-db-01"
name = "Production Database"
host = "localhost"
port = 3306
username = "root"
password = "password"
databases = []

[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

#### Required Fields

- `key`: Unique identifier for this data source (used by agents to reference it)
- `name`: Human-readable name for this data source
- `host`: MySQL server hostname or IP address
- `port`: MySQL server port (typically 3306)
- `username`: MySQL username
- `password`: MySQL password (can be plain text or environment variable reference)

#### Optional Fields

- `databases`: List of database names that are accessible through this data source. Empty list means all databases are accessible.

#### Connection Pool Configuration

Each data source has its own connection pool with the following settings:

- `max_connections`: Maximum number of connections in the pool (default: 10)
- `min_connections`: Minimum number of connections to maintain (default: 2)
- `connection_timeout_secs`: Timeout for acquiring a connection from the pool (default: 30)
- `idle_timeout_secs`: Time before idle connections are closed (default: 300)
- `max_lifetime_secs`: Maximum lifetime of a connection before it's recreated (default: 1800)

## Environment Variables

For security, it's recommended to store sensitive information like passwords in environment variables rather than in the configuration file.

### Using Environment Variables

To use an environment variable for a password, prefix the variable name with `$`:

```toml
[[data_sources]]
key = "prod-db-01"
name = "Production Database"
host = "localhost"
port = 3306
username = "root"
password = "$MYSQL_PASSWORD"  # Loads from MYSQL_PASSWORD environment variable
databases = []
```

Before starting the server, set the environment variable:

```bash
export MYSQL_PASSWORD="your_secure_password"
mysql-mcp-server --config config.toml
```

### Environment Variable Naming

- Use descriptive names that indicate which data source they belong to
- Use uppercase with underscores (e.g., `PROD_DB_PASSWORD`, `DEV_DB_PASSWORD`)
- Never commit actual passwords to version control

## Configuration Validation

The server validates the configuration at startup and will fail to start if:

- Any required field is missing
- Any field has an invalid value (e.g., port = 0)
- Duplicate data source keys are found
- Referenced environment variables are not set
- Pool configuration is invalid (e.g., min_connections > max_connections)

## Example Configurations

### Single Data Source

```toml
query_timeout_secs = 30
stream_chunk_size = 1000

[[data_sources]]
key = "main-db"
name = "Main Database"
host = "localhost"
port = 3306
username = "app_user"
password = "$DB_PASSWORD"
databases = []

[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

### Multiple Data Sources

```toml
query_timeout_secs = 30
stream_chunk_size = 1000

# Production database
[[data_sources]]
key = "prod-db"
name = "Production Database"
host = "prod.db.example.com"
port = 3306
username = "prod_user"
password = "$PROD_DB_PASSWORD"
databases = []

[data_sources.pool_config]
max_connections = 20
min_connections = 5
connection_timeout_secs = 60
idle_timeout_secs = 600
max_lifetime_secs = 3600

# Development database
[[data_sources]]
key = "dev-db"
name = "Development Database"
host = "localhost"
port = 3307
username = "dev_user"
password = "$DEV_DB_PASSWORD"
databases = ["test_db", "dev_db"]

[data_sources.pool_config]
max_connections = 5
min_connections = 1
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

### Restricted Database Access

To restrict access to specific databases only:

```toml
[[data_sources]]
key = "restricted-db"
name = "Restricted Database"
host = "localhost"
port = 3306
username = "limited_user"
password = "$DB_PASSWORD"
databases = ["app_db", "analytics_db"]  # Only these databases are accessible

[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

## Best Practices

1. **Use Environment Variables for Passwords**: Never store passwords in plain text in configuration files
2. **Unique Data Source Keys**: Use descriptive, unique keys for each data source
3. **Appropriate Pool Sizes**: Configure pool sizes based on expected load
4. **Database Restrictions**: Use the `databases` field to limit access to only necessary databases
5. **Version Control**: Add `config.toml` to `.gitignore` and commit `config.example.toml` instead
6. **Documentation**: Document which environment variables need to be set for your configuration

## Troubleshooting

### Configuration File Not Found

```
Error: Failed to read file 'config.toml': No such file or directory
```

**Solution**: Ensure the configuration file exists in the current directory or specify the path with `MCP_CONFIG_PATH` environment variable

### Environment Variable Not Found

```
Error: Environment variable 'MYSQL_PASSWORD' not found
```

**Solution**: Set the environment variable before starting the server:
```bash
export MYSQL_PASSWORD="your_password"
```

### Invalid Configuration

```
Error: Configuration validation error: Data source 'prod-db': host is required
```

**Solution**: Check that all required fields are present and have valid values

### Duplicate Data Source Keys

```
Error: Configuration validation error: Duplicate data source key: prod-db
```

**Solution**: Ensure each data source has a unique `key` value
