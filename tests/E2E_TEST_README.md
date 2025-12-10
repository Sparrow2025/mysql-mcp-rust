# End-to-End Integration Tests

This directory contains comprehensive end-to-end integration tests for the MySQL MCP Server. These tests use a real MySQL database to validate the complete system functionality.

## Prerequisites

- Docker and Docker Compose installed
- Rust toolchain installed

## Running the Tests

### Option 1: Using Docker Compose (Recommended)

1. Start the MySQL test container:
```bash
docker-compose -f docker-compose.test.yml up -d
```

2. Wait for MySQL to be ready (check health status):
```bash
docker-compose -f docker-compose.test.yml ps
```

3. Run the end-to-end tests:
```bash
MYSQL_HOST=localhost MYSQL_PORT=3306 MYSQL_USER=root MYSQL_PASSWORD=testpass cargo test --test e2e_integration_test -- --test-threads=1
```

4. Stop and clean up the test container:
```bash
docker-compose -f docker-compose.test.yml down -v
```

### Option 2: Using Existing MySQL Instance

If you have an existing MySQL instance, you can run the tests against it:

```bash
MYSQL_HOST=your-host \
MYSQL_PORT=3306 \
MYSQL_USER=your-user \
MYSQL_PASSWORD=your-password \
cargo test --test e2e_integration_test -- --test-threads=1
```

**Note:** The tests will create and drop test databases. Make sure the user has appropriate permissions.

## Test Coverage

The end-to-end integration tests cover:

### 1. Complete Query Flow (`test_e2e_complete_query_flow`)
- Creating databases and tables
- Inserting test data
- Executing SELECT queries
- Queries with WHERE clauses
- Aggregation queries

### 2. Multi-Datasource Concurrent Access (`test_e2e_multi_datasource_concurrent_access`)
- Multiple data source configurations
- Concurrent query execution
- Data source isolation
- Independent result verification

### 3. Error Recovery Scenarios (`test_e2e_error_recovery_scenarios`)
- Non-existent database errors
- Non-existent table errors
- Invalid SQL syntax errors
- Invalid data source key errors
- System recovery after errors

### 4. Streaming Query (`test_e2e_streaming_query`)
- Large result sets (2500+ rows)
- Chunked streaming (1000 rows per chunk)
- Multiple chunk retrieval
- Stream completion detection

### 5. Execute Tool DML Operations (`test_e2e_execute_tool_dml_operations`)
- INSERT statements with last_insert_id
- UPDATE statements
- DELETE statements
- Result verification

### 6. Schema Tools (`test_e2e_schema_tools`)
- Listing tables in a database
- Describing table structure
- Column metadata retrieval
- Primary key information
- Error handling for non-existent tables

### 7. List Tools (`test_e2e_list_tools`)
- Listing all data sources
- Listing databases for a data source
- Error handling for invalid keys

### 8. Connection Stats (`test_e2e_connection_stats`)
- Retrieving stats for specific data source
- Retrieving stats for all data sources
- Connection pool metrics

### 9. Multi-Statement Handling (`test_e2e_multi_statement_handling`)
- Executing only the first statement
- Preventing unintended operations
- Security validation

## Test Execution Notes

- Tests are run with `--test-threads=1` to avoid database conflicts
- Each test creates its own test database to ensure isolation
- Tests automatically skip if MySQL is not available
- All test databases are cleaned up after execution

## Environment Variables

- `MYSQL_HOST`: MySQL server hostname (default: localhost)
- `MYSQL_PORT`: MySQL server port (default: 3306)
- `MYSQL_USER`: MySQL username (default: root)
- `MYSQL_PASSWORD`: MySQL password (default: testpass)

## Troubleshooting

### Tests are skipped
If you see "Skipping test: MySQL not available", ensure:
1. MySQL container is running: `docker-compose -f docker-compose.test.yml ps`
2. MySQL is healthy: Check the health status in the output
3. Environment variables are set correctly

### Connection refused errors
- Wait a few seconds after starting the container for MySQL to fully initialize
- Check if port 3306 is already in use: `lsof -i :3306` (macOS/Linux)

### Permission errors
- Ensure the MySQL user has CREATE, DROP, INSERT, UPDATE, DELETE, and SELECT permissions
- For Docker setup, the root user has all necessary permissions

## CI/CD Integration

To integrate these tests into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Start MySQL
  run: docker-compose -f docker-compose.test.yml up -d

- name: Wait for MySQL
  run: |
    timeout 60 bash -c 'until docker-compose -f docker-compose.test.yml exec -T mysql-test mysqladmin ping -h localhost -u root -ptestpass; do sleep 2; done'

- name: Run E2E Tests
  run: |
    MYSQL_HOST=localhost \
    MYSQL_PORT=3306 \
    MYSQL_USER=root \
    MYSQL_PASSWORD=testpass \
    cargo test --test e2e_integration_test -- --test-threads=1

- name: Cleanup
  if: always()
  run: docker-compose -f docker-compose.test.yml down -v
```

## Performance Considerations

- Tests create and populate databases, which can take time
- Large dataset tests (streaming) insert 2500+ rows
- Consider running E2E tests separately from unit tests in CI/CD
- Use `--test-threads=1` to prevent database contention

## Future Enhancements

Potential additions to the E2E test suite:
- Connection pool exhaustion scenarios
- Network interruption simulation
- Long-running query timeout validation
- Resource cleanup verification
- MCP protocol compliance testing
- Performance benchmarking
