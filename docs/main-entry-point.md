# Main Entry Point Implementation

## Overview

The main entry point (`src/main.rs`) implements the server startup, signal handling, and graceful shutdown logic for the MySQL MCP Server.

## Key Features

### 1. Configuration Loading

The server loads configuration from a TOML file specified by:
- The `MCP_CONFIG_PATH` environment variable, or
- Defaults to `config.toml` in the current directory

Configuration includes:
- Data source definitions (host, port, credentials)
- Connection pool settings
- Query timeout and stream chunk size

### 2. Signal Handling

The server handles the following signals for graceful shutdown:

**On Unix systems (Linux, macOS):**
- `SIGTERM` - Termination signal (e.g., from `kill` command)
- `SIGINT` - Interrupt signal (Ctrl+C)

**On Windows:**
- `Ctrl+C` - Console interrupt

### 3. Graceful Shutdown

When a shutdown signal is received, the server:

1. **Stops accepting new requests** - The MCP service is dropped
2. **Closes all connection pools** - All active database connections are closed gracefully
3. **Stops monitoring service** - Background statistics logging is stopped
4. **Logs shutdown progress** - Each step is logged for observability

### 4. Resource Cleanup

The `MySqlMcpServerHandler::cleanup()` method ensures:
- All connection pools are closed via `pool_manager.close_all()`
- The monitoring service is stopped
- Resources are released properly

## Implementation Details

### Async Runtime

The server uses Tokio as the async runtime:
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ...
}
```

### Signal Handler Setup

The `setup_signal_handlers()` function creates a oneshot channel that completes when a shutdown signal is received:

```rust
fn setup_signal_handlers() -> tokio::sync::oneshot::Receiver<()>
```

This allows the main function to wait for the signal asynchronously.

### MCP Service Lifecycle

1. **Initialization**: `MySqlMcpServerHandler::new(config)` creates the handler
2. **Serving**: `handler.serve(transport)` starts the MCP service
3. **Shutdown**: Dropping the running service stops it
4. **Cleanup**: `handler.cleanup()` releases resources

## Requirements Satisfied

This implementation satisfies **Requirement 5.4**:
> WHEN the MCP Server shuts down, THE MCP Server SHALL close all active connections gracefully

## Usage

### Starting the Server

```bash
# Using default config.toml
cargo run --release

# Using custom config file
MCP_CONFIG_PATH=/path/to/config.toml cargo run --release
```

### Stopping the Server

**Graceful shutdown:**
```bash
# Send SIGTERM (Unix)
kill <pid>

# Or press Ctrl+C in the terminal
```

The server will:
1. Log "Shutdown signal received"
2. Stop the MCP service
3. Close all connection pools
4. Log "MySQL MCP Server shutdown complete"

## Logging

The server uses structured logging with trace IDs:
- Server session trace ID is generated at startup
- All operations are logged with appropriate levels (INFO, WARN, ERROR)
- Sensitive information (passwords, credentials) is automatically filtered

## Error Handling

- Configuration errors are reported with clear messages
- Missing configuration files result in helpful error messages
- All errors are logged before the server exits
