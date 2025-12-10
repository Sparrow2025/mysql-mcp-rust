# Task 14: Logging and Monitoring Implementation

## Overview

This document describes the implementation of logging and monitoring features for the MySQL MCP Server, including:
- Tracing subscriber configuration with sensitive information filtering
- Periodic connection pool statistics logging
- Structured logging with trace IDs
- Different log levels for various operations

## Implementation Details

### 1. Logging Module (`src/logging/mod.rs`)

Created a dedicated logging module that provides:

#### Sensitive Information Filtering
- Implements a custom `SanitizingLayer` that intercepts log events
- Uses the existing `sanitize_error_message` function from the error module
- Automatically detects and redacts credentials, passwords, and connection strings
- Logs a warning when sensitive information is detected and sanitized

#### Tracing Initialization
- Configures `tracing-subscriber` with environment-based filtering
- Default log level: `mysql_mcp_server=info`
- Can be overridden with `RUST_LOG` environment variable
- Includes thread IDs and names for better debugging
- Logs to stderr to avoid interfering with MCP protocol on stdout

**Key Features:**
```rust
pub fn init_tracing() {
    // Custom formatter with sanitization
    // Thread information included
    // Span events tracked
    // Environment-based filtering
}
```

### 2. Monitoring Module (`src/monitoring/mod.rs`)

Created a monitoring service that provides periodic statistics logging:

#### MonitoringService
- Spawns a background task that runs every 60 seconds
- Logs connection pool statistics for all data sources
- Includes:
  - Data source key and status
  - Database name
  - Active connections count
  - Idle connections count
  - Total connections count

#### Features
- Non-blocking background task using tokio
- Graceful shutdown support
- Automatic cleanup on drop
- Skips missed ticks to avoid backlog

**Usage:**
```rust
let service = MonitoringService::new(manager, pool_managers, 60).start();
// Service runs in background
// Automatically stops when dropped
```

### 3. Structured Logging with Trace IDs

#### Server Session Trace ID
- Each server session gets a unique UUID trace ID
- Logged at server startup in `main.rs`
- Helps correlate all logs from a single server run

#### Tool Call Trace IDs
- Each tool invocation gets a unique UUID trace ID
- Logged at the start and end of each tool call
- Includes:
  - Tool name
  - Success/failure status
  - Error messages (sanitized)

#### Resource Read Trace IDs
- Each resource read gets a unique UUID trace ID
- Logged at the start and end of each resource read
- Includes:
  - Resource URI
  - Success/failure status
  - Error messages (sanitized)

**Example Log Output:**
```
INFO server_session{trace_id=550e8400-e29b-41d4-a716-446655440000}: MySQL MCP Server starting...
INFO trace_id=123e4567-e89b-12d3-a456-426614174000 tool_name=mysql_query: Tool call started
INFO trace_id=123e4567-e89b-12d3-a456-426614174000: Tool call completed successfully
```

### 4. Log Levels Implementation

The implementation uses different log levels appropriately:

#### ERROR Level
- Connection failures
- Query execution errors
- Tool call failures
- Resource read failures
- Configuration errors

#### WARN Level
- Sensitive information detected in logs
- Data sources marked as unavailable
- Retry attempts

#### INFO Level
- Server startup/shutdown
- Configuration loaded
- Tool calls (start/end)
- Resource reads (start/end)
- Query execution (with timing)
- Connection pool statistics (periodic)
- Data source registration

#### DEBUG Level
- Health check results
- Reconnection attempts
- Tool/resource listing operations

#### TRACE Level
- Detailed execution flow (via span events)

### 5. Integration Points

#### Main Entry Point (`src/main.rs`)
- Calls `init_tracing()` at startup
- Creates server session trace ID
- Uses structured logging throughout

#### MCP Server Handler (`src/mcp_server.rs`)
- Integrates MonitoringService
- Adds trace IDs to tool calls and resource reads
- Sanitizes error messages before returning to client
- Logs operation start/end with timing

#### Tools Module (`src/tools/mod.rs`)
- Logs query execution with timing
- Logs data source and database information
- Logs row counts and execution times
- Sanitizes errors before logging

#### Error Module (`src/error/mod.rs`)
- Provides `sanitize()` method on all errors
- Used throughout the codebase for safe logging
- Comprehensive regex patterns for credential detection

### 6. Configuration

#### Environment Variables
- `RUST_LOG`: Controls log level (e.g., `RUST_LOG=debug`)
- `MCP_CONFIG_PATH`: Path to configuration file

#### Log Output
- All logs go to stderr
- MCP protocol communication uses stdout
- Structured format with key-value pairs

### 7. Testing

All modules include comprehensive tests:

#### Logging Module Tests
- Verifies sanitization logic is available
- Tests integration with error module

#### Monitoring Module Tests
- Tests service creation
- Tests start/stop functionality
- Verifies background task management

#### Error Module Tests
- Tests credential sanitization patterns
- Tests various connection string formats
- Tests error message sanitization

## Requirements Validation

### Requirement 5.5
✅ **"THE MCP Server SHALL log connection pool statistics every 60 seconds for monitoring purposes"**
- Implemented via `MonitoringService`
- Logs every 60 seconds in background task
- Includes all relevant statistics

### Requirement 10.3
✅ **"WHEN logging operations, THE MCP Server SHALL mask all sensitive information including passwords and connection strings"**
- Implemented via `SanitizingLayer` in logging module
- Uses comprehensive regex patterns from error module
- Automatically detects and redacts credentials
- Logs warning when sensitive information is found

## Additional Features

### Thread Safety
- All logging is thread-safe via tracing
- Monitoring service uses Arc and RwLock for shared state
- No blocking operations in logging paths

### Performance
- Minimal overhead from sanitization layer
- Background monitoring task doesn't block main operations
- Efficient regex compilation (compiled once, reused)

### Observability
- Trace IDs enable request tracing
- Structured logging enables easy parsing
- Timing information for performance analysis
- Connection pool metrics for capacity planning

## Usage Examples

### Starting the Server
```bash
# Default log level (info)
cargo run

# Debug log level
RUST_LOG=debug cargo run

# Trace log level for specific module
RUST_LOG=mysql_mcp_server::tools=trace cargo run
```

### Log Output Examples

**Server Startup:**
```
INFO Tracing initialized with sensitive information filtering
INFO server_session{trace_id=...}: MySQL MCP Server starting...
INFO datasource_count=2: Configuration loaded successfully
INFO server_name=mysql-mcp-server version=0.1.0: MCP server handler created successfully
INFO interval_secs=60: Monitoring service started
```

**Tool Call:**
```
INFO trace_id=... tool_name=mysql_query: Tool call started
INFO datasource_key=prod-db database=users query_length=45: Executing query
INFO datasource_key=prod-db database=users rows=10 execution_time_ms=123: Query executed successfully
INFO trace_id=...: Tool call completed successfully
```

**Periodic Statistics:**
```
INFO datasource_count=2: Connection pool statistics report
INFO datasource_key=prod-db database=users status=Available active_connections=2 idle_connections=3 total_connections=5: Connection pool statistics
INFO datasource_key=dev-db database=test status=Available active_connections=0 idle_connections=2 total_connections=2: Connection pool statistics
```

**Error with Sanitization:**
```
ERROR trace_id=... error="Connection failed: [REDACTED]": Tool call failed
WARN Sensitive information detected and sanitized in log message
```

## Future Enhancements

Potential improvements for future iterations:

1. **Metrics Export**: Export metrics to Prometheus or similar
2. **Log Aggregation**: Integration with log aggregation services
3. **Alert Thresholds**: Configurable alerts for connection pool exhaustion
4. **Performance Profiling**: Detailed timing breakdowns for slow queries
5. **Audit Logging**: Separate audit trail for compliance
6. **Log Rotation**: Built-in log rotation for file-based logging

## Conclusion

The logging and monitoring implementation provides comprehensive observability for the MySQL MCP Server while ensuring that sensitive information is never exposed in logs. The structured logging with trace IDs enables effective debugging and performance analysis, while the periodic statistics logging provides operational visibility into connection pool health.
