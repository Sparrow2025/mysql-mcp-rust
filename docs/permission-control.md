# Permission Control

## Overview

The MySQL MCP Server implements a permission control system that allows you to restrict what operations can be performed on each data source. This provides fine-grained access control without exposing database credentials.

## Permission Levels

There are three permission levels, each building on the previous:

### 1. Query (Read-Only)
- **Allowed Operations**: SELECT queries only
- **Denied Operations**: INSERT, UPDATE, DELETE, and all DDL operations
- **Use Case**: Read-only access for reporting, analytics, or data exploration

### 2. Update (Read-Write)
- **Allowed Operations**: SELECT, INSERT, UPDATE, DELETE
- **Denied Operations**: DDL operations (CREATE, ALTER, DROP, TRUNCATE, RENAME)
- **Use Case**: Application data access where schema changes should be restricted

### 3. DDL (Full Access)
- **Allowed Operations**: All SQL operations including DDL
- **Denied Operations**: None
- **Use Case**: Database administration, schema migrations, full control

## Configuration

### Setting Permissions

Add the `permission` field to each data source in your configuration file:

```toml
[[data_sources]]
key = "readonly-db"
name = "Read-Only Database"
host = "localhost"
port = 3306
username = "readonly_user"
password = "$READONLY_PASSWORD"
permission = "query"  # Read-only access

[[data_sources]]
key = "app-db"
name = "Application Database"
host = "localhost"
port = 3306
username = "app_user"
password = "$APP_PASSWORD"
permission = "update"  # Read-write access, no DDL

[[data_sources]]
key = "admin-db"
name = "Admin Database"
host = "localhost"
port = 3306
username = "admin_user"
password = "$ADMIN_PASSWORD"
permission = "ddl"  # Full access including DDL
```

### Default Permission

If the `permission` field is omitted, the default is `query` (read-only access).

## How It Works

### Query Tool (`mysql_query`)

The query tool checks for `query` permission before executing any SELECT statement:

```rust
// Permission check happens automatically
manager.check_query_permission(datasource_key)?;
```

**Example:**
- ✅ `SELECT * FROM users` - Allowed with `query`, `update`, or `ddl` permission
- ❌ `INSERT INTO users VALUES (...)` - Not allowed through query tool

### Execute Tool (`mysql_execute`)

The execute tool checks permissions based on the statement type:

1. **For DML statements** (INSERT, UPDATE, DELETE):
   - Requires `update` or `ddl` permission
   - Denied with `query` permission

2. **For DDL statements** (CREATE, ALTER, DROP, etc.):
   - Requires `ddl` permission
   - Denied with `query` or `update` permission

```rust
// Permission check based on statement type
if is_ddl_statement(statement) {
    manager.check_ddl_permission(datasource_key)?;
} else {
    manager.check_update_permission(datasource_key)?;
}
```

**Examples:**
- ✅ `INSERT INTO users VALUES (1, 'Alice')` - Allowed with `update` or `ddl`
- ❌ `INSERT INTO users VALUES (1, 'Alice')` - Denied with `query`
- ✅ `CREATE TABLE products (id INT)` - Allowed with `ddl` only
- ❌ `CREATE TABLE products (id INT)` - Denied with `query` or `update`

## Error Messages

When a permission check fails, the server returns a `PermissionDenied` error with a descriptive message:

```
Permission denied: Data source 'readonly-db' does not have update permission (current: Query)
```

```
Permission denied: Data source 'app-db' does not have DDL permission (current: Update)
```

## Security Benefits

1. **Principle of Least Privilege**: Grant only the minimum permissions needed for each use case
2. **Defense in Depth**: Even if a data source key is compromised, the attacker is limited by the permission level
3. **Audit Trail**: Permission checks are logged, making it easier to track unauthorized access attempts
4. **Separation of Concerns**: Different keys can have different permissions to the same database

## Best Practices

1. **Use Query Permission by Default**: Start with read-only access and grant higher permissions only when needed

2. **Separate Keys for Different Purposes**:
   ```toml
   # Analytics team - read-only
   [[data_sources]]
   key = "analytics-readonly"
   permission = "query"
   
   # Application - read-write
   [[data_sources]]
   key = "app-readwrite"
   permission = "update"
   
   # DBA - full access
   [[data_sources]]
   key = "dba-admin"
   permission = "ddl"
   ```

3. **Document Permission Levels**: Clearly document which keys have which permissions in your team's documentation

4. **Regular Audits**: Periodically review permission assignments to ensure they're still appropriate

5. **Use Environment Variables**: Store sensitive credentials in environment variables, not in the configuration file

## Examples

### Example 1: Read-Only Analytics

```toml
[[data_sources]]
key = "analytics"
name = "Analytics Database"
host = "analytics.example.com"
port = 3306
username = "analyst"
password = "$ANALYTICS_PASSWORD"
permission = "query"
```

Agents using this key can:
- ✅ Run SELECT queries
- ✅ Generate reports
- ✅ Explore data

But cannot:
- ❌ Modify data
- ❌ Create or alter tables

### Example 2: Application Database

```toml
[[data_sources]]
key = "webapp"
name = "Web Application Database"
host = "db.example.com"
port = 3306
username = "webapp_user"
password = "$WEBAPP_PASSWORD"
permission = "update"
```

Agents using this key can:
- ✅ Read data (SELECT)
- ✅ Insert new records
- ✅ Update existing records
- ✅ Delete records

But cannot:
- ❌ Create new tables
- ❌ Alter table schemas
- ❌ Drop tables

### Example 3: Database Administration

```toml
[[data_sources]]
key = "dba"
name = "DBA Access"
host = "db.example.com"
port = 3306
username = "dba_user"
password = "$DBA_PASSWORD"
permission = "ddl"
```

Agents using this key can:
- ✅ Perform all operations
- ✅ Create and modify schemas
- ✅ Run migrations
- ✅ Full database control

## Testing

The permission system includes comprehensive tests to ensure correct behavior:

```bash
# Run permission tests
cargo test --test permission_test

# Run all tests
cargo test --all
```

## API Reference

### Permission Enum

```rust
pub enum Permission {
    Query,   // Read-only
    Update,  // Read-write (DML)
    Ddl,     // Full access
}
```

### Permission Check Methods

```rust
// Check if a data source has query permission
manager.check_query_permission(key: &str) -> Result<()>

// Check if a data source has update permission
manager.check_update_permission(key: &str) -> Result<()>

// Check if a data source has DDL permission
manager.check_ddl_permission(key: &str) -> Result<()>

// Get the permission level for a data source
manager.get_permission(key: &str) -> Option<Permission>
```

## Troubleshooting

### "Permission denied" errors

If you receive permission denied errors:

1. Check the permission level in your configuration file
2. Verify you're using the correct data source key
3. Ensure the operation matches the permission level:
   - SELECT → requires `query` or higher
   - INSERT/UPDATE/DELETE → requires `update` or higher
   - CREATE/ALTER/DROP → requires `ddl`

### Permission not taking effect

1. Restart the MCP server after changing the configuration
2. Verify the configuration file syntax is correct
3. Check the server logs for configuration errors

## Migration Guide

If you have an existing configuration without permissions:

1. **Backup your configuration file**

2. **Add permission fields** to each data source:
   ```toml
   [[data_sources]]
   key = "existing-db"
   # ... other fields ...
   permission = "query"  # Start with read-only
   ```

3. **Test thoroughly** before deploying to production

4. **Gradually increase permissions** as needed

The default permission is `query` (read-only), so existing configurations will continue to work but with restricted access until you explicitly grant higher permissions.
