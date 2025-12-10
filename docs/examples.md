# Usage Examples

This document provides practical examples of using the MySQL MCP Server.

## Table of Contents

- [Basic Query Examples](#basic-query-examples)
- [Schema Inspection](#schema-inspection)
- [Data Modification](#data-modification)
- [Streaming Large Results](#streaming-large-results)
- [Resource Access](#resource-access)
- [Monitoring](#monitoring)
- [Error Handling](#error-handling)

## Basic Query Examples

### Simple SELECT Query

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "query": "SELECT id, username, email FROM accounts WHERE status = 'active' LIMIT 10"
  }
}
```

**Response**:
```json
{
  "columns": [
    {"name": "id", "data_type": "INT", "nullable": false},
    {"name": "username", "data_type": "VARCHAR", "nullable": false},
    {"name": "email", "data_type": "VARCHAR", "nullable": true}
  ],
  "rows": [
    {"id": 1, "username": "john_doe", "email": "john@example.com"},
    {"id": 2, "username": "jane_smith", "email": "jane@example.com"}
  ],
  "affected_rows": 0,
  "execution_time": "15ms"
}
```

### Query with WHERE Clause

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "sales",
    "query": "SELECT order_id, total, created_at FROM orders WHERE created_at >= '2024-01-01' AND status = 'completed'"
  }
}
```

### Aggregate Query

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "analytics",
    "query": "SELECT DATE(created_at) as date, COUNT(*) as count, SUM(amount) as total FROM transactions GROUP BY DATE(created_at) ORDER BY date DESC LIMIT 30"
  }
}
```

### JOIN Query

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "ecommerce",
    "query": "SELECT o.order_id, o.total, c.name, c.email FROM orders o JOIN customers c ON o.customer_id = c.id WHERE o.status = 'pending'"
  }
}
```

## Schema Inspection

### List All Data Sources

```json
{
  "tool": "mysql_list_datasources",
  "arguments": {}
}
```

**Response**:
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

### List Databases

```json
{
  "tool": "mysql_list_databases",
  "arguments": {
    "datasource_key": "prod-db-01"
  }
}
```

**Response**:
```json
[
  {
    "name": "users",
    "size_bytes": 104857600,
    "charset": "utf8mb4",
    "collation": "utf8mb4_unicode_ci"
  },
  {
    "name": "sales",
    "size_bytes": 524288000,
    "charset": "utf8mb4",
    "collation": "utf8mb4_unicode_ci"
  }
]
```

### List Tables

```json
{
  "tool": "mysql_list_tables",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users"
  }
}
```

**Response**:
```json
[
  {
    "name": "accounts",
    "row_count": 15420,
    "size_bytes": 2097152,
    "engine": "InnoDB"
  },
  {
    "name": "profiles",
    "row_count": 15420,
    "size_bytes": 4194304,
    "engine": "InnoDB"
  }
]
```

### Describe Table Structure

```json
{
  "tool": "mysql_describe_table",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "table": "accounts"
  }
}
```

**Response**:
```json
{
  "table_name": "accounts",
  "columns": [
    {
      "name": "id",
      "data_type": "INT",
      "nullable": false,
      "default_value": null,
      "comment": "Primary key"
    },
    {
      "name": "username",
      "data_type": "VARCHAR(255)",
      "nullable": false,
      "default_value": null,
      "comment": "Unique username"
    },
    {
      "name": "email",
      "data_type": "VARCHAR(255)",
      "nullable": true,
      "default_value": null,
      "comment": "User email address"
    },
    {
      "name": "created_at",
      "data_type": "TIMESTAMP",
      "nullable": false,
      "default_value": "CURRENT_TIMESTAMP",
      "comment": null
    }
  ],
  "primary_key": ["id"],
  "foreign_keys": [],
  "indexes": [
    {
      "name": "idx_username",
      "columns": ["username"],
      "unique": true,
      "index_type": "BTREE"
    },
    {
      "name": "idx_email",
      "columns": ["email"],
      "unique": false,
      "index_type": "BTREE"
    }
  ]
}
```

## Data Modification

### INSERT Statement

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "dev-db-01",
    "database": "test_db",
    "statement": "INSERT INTO users (username, email, status) VALUES ('new_user', 'new@example.com', 'active')"
  }
}
```

**Response**:
```json
{
  "affected_rows": 1,
  "last_insert_id": 12345
}
```

### UPDATE Statement

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "dev-db-01",
    "database": "test_db",
    "statement": "UPDATE users SET status = 'inactive' WHERE last_login < '2023-01-01'"
  }
}
```

**Response**:
```json
{
  "affected_rows": 42,
  "last_insert_id": 0
}
```

### DELETE Statement

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "dev-db-01",
    "database": "test_db",
    "statement": "DELETE FROM temp_data WHERE created_at < DATE_SUB(NOW(), INTERVAL 30 DAY)"
  }
}
```

**Response**:
```json
{
  "affected_rows": 156,
  "last_insert_id": 0
}
```

### Bulk INSERT

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "dev-db-01",
    "database": "test_db",
    "statement": "INSERT INTO logs (level, message, created_at) VALUES ('INFO', 'System started', NOW()), ('INFO', 'Configuration loaded', NOW()), ('INFO', 'Ready to accept connections', NOW())"
  }
}
```

**Response**:
```json
{
  "affected_rows": 3,
  "last_insert_id": 1001
}
```

## Streaming Large Results

### Enable Streaming for Large Result Sets

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "analytics",
    "query": "SELECT * FROM events WHERE created_at >= '2024-01-01'",
    "stream": true
  }
}
```

The server will send results in chunks of 1000 rows (configurable):

**First Chunk**:
```json
{
  "chunk": 1,
  "columns": [...],
  "rows": [/* 1000 rows */],
  "has_more": true
}
```

**Subsequent Chunks**:
```json
{
  "chunk": 2,
  "rows": [/* 1000 rows */],
  "has_more": true
}
```

**Final Chunk**:
```json
{
  "chunk": 5,
  "rows": [/* remaining rows */],
  "has_more": false
}
```

## Resource Access

### Access Data Sources via Resource URI

```
mysql://datasources
```

**Response**:
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

### Access Databases via Resource URI

```
mysql://prod-db-01/databases
```

**Response**:
```json
{
  "databases": [
    {"name": "users", "size_bytes": 104857600},
    {"name": "sales", "size_bytes": 524288000}
  ]
}
```

### Access Tables via Resource URI

```
mysql://prod-db-01/users/tables
```

**Response**:
```json
{
  "tables": [
    {"name": "accounts", "row_count": 15420},
    {"name": "profiles", "row_count": 15420}
  ]
}
```

### Access Table Schema via Resource URI

```
mysql://prod-db-01/users/tables/accounts
```

**Response**: Full table schema (same as `mysql_describe_table`)

### Access Complete Database Schema

```
mysql://prod-db-01/users/schema
```

**Response**: Complete schema for all tables in the database

## Monitoring

### Get Connection Statistics for All Data Sources

```json
{
  "tool": "mysql_get_connection_stats",
  "arguments": {}
}
```

**Response**:
```json
[
  {
    "datasource_key": "prod-db-01",
    "active_connections": 5,
    "idle_connections": 3,
    "total_connections": 8,
    "queued_requests": 0
  },
  {
    "datasource_key": "dev-db-01",
    "active_connections": 1,
    "idle_connections": 1,
    "total_connections": 2,
    "queued_requests": 0
  }
]
```

### Get Connection Statistics for Specific Data Source

```json
{
  "tool": "mysql_get_connection_stats",
  "arguments": {
    "datasource_key": "prod-db-01"
  }
}
```

**Response**:
```json
{
  "datasource_key": "prod-db-01",
  "active_connections": 5,
  "idle_connections": 3,
  "total_connections": 8,
  "queued_requests": 0,
  "max_connections": 10,
  "min_connections": 2
}
```

## Error Handling

### Invalid Data Source Key

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "invalid-key",
    "database": "users",
    "query": "SELECT * FROM accounts"
  }
}
```

**Error Response**:
```json
{
  "error_code": "AUTHENTICATION_ERROR",
  "message": "Invalid data source key: invalid-key"
}
```

### Database Not Found

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "nonexistent_db",
    "query": "SELECT * FROM accounts"
  }
}
```

**Error Response**:
```json
{
  "error_code": "DATABASE_NOT_FOUND",
  "message": "Database 'nonexistent_db' does not exist"
}
```

### SQL Syntax Error

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "query": "SELCT * FROM accounts"
  }
}
```

**Error Response**:
```json
{
  "error_code": "QUERY_EXECUTION_ERROR",
  "message": "SQL syntax error: You have an error in your SQL syntax near 'SELCT'"
}
```

### Query Timeout

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "analytics",
    "query": "SELECT * FROM huge_table WHERE complex_calculation(column) > 1000"
  }
}
```

**Error Response**:
```json
{
  "error_code": "QUERY_TIMEOUT",
  "message": "Query execution exceeded timeout of 30 seconds"
}
```

### DDL Statement Rejected

```json
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "statement": "DROP TABLE accounts"
  }
}
```

**Error Response**:
```json
{
  "error_code": "DDL_NOT_ALLOWED",
  "message": "DDL statements (CREATE, ALTER, DROP) are not allowed. Use mysql_query for read operations only."
}
```

### Connection Pool Exhausted

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "query": "SELECT * FROM accounts"
  }
}
```

**Error Response** (when all connections are in use):
```json
{
  "error_code": "CONNECTION_TIMEOUT",
  "message": "Failed to acquire connection from pool within timeout period"
}
```

## Advanced Examples

### Parameterized Query Pattern

While the server doesn't support parameterized queries directly, you can construct safe queries:

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "query": "SELECT * FROM accounts WHERE username = 'john_doe' AND status = 'active'"
  }
}
```

**Note**: Always sanitize inputs on the client side to prevent SQL injection.

### Transaction Pattern

Execute multiple statements as separate calls:

```json
// Step 1: Update account
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "banking",
    "statement": "UPDATE accounts SET balance = balance - 100 WHERE account_id = 123"
  }
}

// Step 2: Insert transaction record
{
  "tool": "mysql_execute",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "banking",
    "statement": "INSERT INTO transactions (account_id, amount, type) VALUES (123, -100, 'withdrawal')"
  }
}
```

**Note**: Each execute call is auto-committed. For true transactions, consider using stored procedures.

### Pagination Pattern

```json
// Page 1
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "query": "SELECT * FROM accounts ORDER BY id LIMIT 100 OFFSET 0"
  }
}

// Page 2
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "users",
    "query": "SELECT * FROM accounts ORDER BY id LIMIT 100 OFFSET 100"
  }
}
```

### Search Pattern

```json
{
  "tool": "mysql_query",
  "arguments": {
    "datasource_key": "prod-db-01",
    "database": "products",
    "query": "SELECT * FROM items WHERE name LIKE '%laptop%' OR description LIKE '%laptop%' ORDER BY relevance DESC LIMIT 20"
  }
}
```

## Best Practices

1. **Use Streaming for Large Results**: Enable streaming when expecting more than 1000 rows
2. **Limit Result Sets**: Always use LIMIT clauses to prevent accidentally fetching millions of rows
3. **Index Your Queries**: Ensure frequently queried columns have appropriate indexes
4. **Monitor Connection Pools**: Regularly check connection statistics to optimize pool sizes
5. **Handle Errors Gracefully**: Always check for error responses and handle them appropriately
6. **Use Appropriate Data Sources**: Use development data sources for testing, production for live data
7. **Sanitize Inputs**: Always validate and sanitize user inputs before constructing queries
8. **Avoid DDL in Production**: Never use DDL statements through the execute tool in production
