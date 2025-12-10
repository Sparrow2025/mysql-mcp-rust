# End-to-End Testing Guide

This guide provides comprehensive information about running and maintaining the end-to-end integration tests for the MySQL MCP Server.

## Overview

The E2E tests validate the complete system functionality using a real MySQL database. These tests ensure that all components work together correctly in a production-like environment.

## Quick Start

### Using the Helper Script (Recommended)

The easiest way to run E2E tests is using the provided script:

```bash
./run_e2e_tests.sh
```

This script will:
1. Check Docker prerequisites
2. Start a MySQL test container
3. Wait for MySQL to be ready
4. Run all E2E tests
5. Clean up the container automatically

### Manual Execution

If you prefer manual control:

```bash
# Start MySQL container
docker-compose -f docker-compose.test.yml up -d

# Wait for MySQL to be ready
docker-compose -f docker-compose.test.yml exec mysql-test mysqladmin ping -h localhost -u root -ptestpass

# Run tests
MYSQL_HOST=localhost MYSQL_PORT=3306 MYSQL_USER=root MYSQL_PASSWORD=testpass \
  cargo test --test e2e_integration_test -- --test-threads=1

# Clean up
docker-compose -f docker-compose.test.yml down -v
```

## Script Options

The `run_e2e_tests.sh` script supports several options:

### Start Only
Start the MySQL container without running tests:
```bash
./run_e2e_tests.sh --start-only
```

### Stop Only
Stop and clean up the MySQL container:
```bash
./run_e2e_tests.sh --stop-only
```

### Skip Cleanup
Keep the container running after tests (useful for debugging):
```bash
./run_e2e_tests.sh --skip-cleanup
```

### Help
Display usage information:
```bash
./run_e2e_tests.sh --help
```

## Test Suite Details

### Test 1: Complete Query Flow
**File:** `test_e2e_complete_query_flow`

Tests the basic query execution pipeline:
- Database and table creation
- Data insertion
- SELECT queries
- WHERE clause filtering
- Aggregation functions

**Validates Requirements:** 1.1, 1.4, 2.3, 3.1, 3.2

### Test 2: Multi-Datasource Concurrent Access
**File:** `test_e2e_multi_datasource_concurrent_access`

Tests concurrent access to multiple data sources:
- Multiple data source configurations
- Parallel query execution
- Data source isolation
- Independent result verification

**Validates Requirements:** 1.5, 2.5, 9.5

### Test 3: Error Recovery Scenarios
**File:** `test_e2e_error_recovery_scenarios`

Tests error handling and system resilience:
- Non-existent database errors
- Non-existent table errors
- Invalid SQL syntax
- Invalid data source keys
- System recovery after errors

**Validates Requirements:** 2.4, 3.3, 6.3, 7.1, 7.4, 8.4

### Test 4: Streaming Query
**File:** `test_e2e_streaming_query`

Tests large result set handling:
- Large dataset (2500+ rows)
- Chunked streaming (1000 rows/chunk)
- Multiple chunk retrieval
- Stream completion

**Validates Requirements:** 9.1, 9.2, 9.3, 9.4

### Test 5: Execute Tool DML Operations
**File:** `test_e2e_execute_tool_dml_operations`

Tests data modification operations:
- INSERT with last_insert_id
- UPDATE operations
- DELETE operations
- Result verification

**Validates Requirements:** 11.1, 11.2, 11.5

### Test 6: Schema Tools
**File:** `test_e2e_schema_tools`

Tests schema introspection:
- Table listing
- Table structure description
- Column metadata
- Primary key information
- Error handling

**Validates Requirements:** 4.1, 4.2, 4.3, 4.4, 4.5

### Test 7: List Tools
**File:** `test_e2e_list_tools`

Tests listing functionality:
- Data source listing
- Database listing
- Error handling

**Validates Requirements:** 6.1, 6.2, 6.3

### Test 8: Connection Stats
**File:** `test_e2e_connection_stats`

Tests connection pool monitoring:
- Single data source stats
- All data sources stats
- Pool metrics

**Validates Requirements:** 12.1, 12.2, 12.3, 12.4

### Test 9: Multi-Statement Handling
**File:** `test_e2e_multi_statement_handling`

Tests SQL injection prevention:
- First statement execution only
- Preventing unintended operations
- Security validation

**Validates Requirements:** 3.4

## Environment Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MYSQL_HOST` | localhost | MySQL server hostname |
| `MYSQL_PORT` | 3306 | MySQL server port |
| `MYSQL_USER` | root | MySQL username |
| `MYSQL_PASSWORD` | testpass | MySQL password |

### Custom MySQL Instance

To use a different MySQL instance:

```bash
MYSQL_HOST=my-server.example.com \
MYSQL_PORT=3307 \
MYSQL_USER=testuser \
MYSQL_PASSWORD=mypassword \
cargo test --test e2e_integration_test -- --test-threads=1
```

**Important:** The test user must have permissions to:
- CREATE and DROP databases
- CREATE and DROP tables
- INSERT, UPDATE, DELETE, and SELECT data
- Query information_schema

## Troubleshooting

### Tests Are Skipped

**Symptom:** All tests show "Skipping test: MySQL not available"

**Solutions:**
1. Verify MySQL container is running:
   ```bash
   docker-compose -f docker-compose.test.yml ps
   ```

2. Check MySQL health:
   ```bash
   docker-compose -f docker-compose.test.yml exec mysql-test mysqladmin ping -h localhost -u root -ptestpass
   ```

3. Verify environment variables are set correctly

4. Check if port 3306 is accessible:
   ```bash
   nc -zv localhost 3306
   ```

### Connection Refused

**Symptom:** "Connection refused" errors

**Solutions:**
1. Wait longer for MySQL to initialize (can take 10-30 seconds)
2. Check if another service is using port 3306:
   ```bash
   lsof -i :3306  # macOS/Linux
   netstat -ano | findstr :3306  # Windows
   ```
3. Try restarting the container:
   ```bash
   docker-compose -f docker-compose.test.yml restart
   ```

### Permission Denied

**Symptom:** MySQL permission errors

**Solutions:**
1. Verify the test user has necessary permissions
2. For Docker setup, ensure you're using the root user
3. Check MySQL logs:
   ```bash
   docker-compose -f docker-compose.test.yml logs mysql-test
   ```

### Tests Hang or Timeout

**Symptom:** Tests don't complete or timeout

**Solutions:**
1. Check MySQL container logs for errors
2. Verify network connectivity
3. Increase timeout values in test code
4. Run tests with verbose output:
   ```bash
   cargo test --test e2e_integration_test -- --test-threads=1 --nocapture
   ```

### Database Already Exists

**Symptom:** "Database already exists" errors

**Solutions:**
1. Clean up test databases manually:
   ```bash
   docker-compose -f docker-compose.test.yml exec mysql-test mysql -u root -ptestpass -e "DROP DATABASE IF EXISTS e2e_test_db;"
   ```

2. Restart with clean state:
   ```bash
   docker-compose -f docker-compose.test.yml down -v
   docker-compose -f docker-compose.test.yml up -d
   ```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Start MySQL
        run: docker-compose -f docker-compose.test.yml up -d
        
      - name: Wait for MySQL
        run: |
          timeout 60 bash -c 'until docker-compose -f docker-compose.test.yml exec -T mysql-test mysqladmin ping -h localhost -u root -ptestpass --silent; do sleep 2; done'
          
      - name: Run E2E Tests
        env:
          MYSQL_HOST: localhost
          MYSQL_PORT: 3306
          MYSQL_USER: root
          MYSQL_PASSWORD: testpass
        run: cargo test --test e2e_integration_test -- --test-threads=1
        
      - name: Cleanup
        if: always()
        run: docker-compose -f docker-compose.test.yml down -v
```

### GitLab CI Example

```yaml
e2e-tests:
  image: rust:latest
  services:
    - mysql:8.0
  variables:
    MYSQL_ROOT_PASSWORD: testpass
    MYSQL_DATABASE: testdb
    MYSQL_HOST: mysql
    MYSQL_PORT: "3306"
    MYSQL_USER: root
    MYSQL_PASSWORD: testpass
  script:
    - cargo test --test e2e_integration_test -- --test-threads=1
```

## Performance Considerations

### Test Execution Time

Typical execution times:
- Container startup: 10-30 seconds
- Individual test: 1-5 seconds
- Full suite: 30-60 seconds

### Optimization Tips

1. **Reuse Container:** Use `--skip-cleanup` during development
2. **Parallel Tests:** Not recommended due to database conflicts
3. **Smaller Datasets:** Reduce test data size for faster execution
4. **Selective Testing:** Run specific tests during development:
   ```bash
   cargo test --test e2e_integration_test test_e2e_complete_query_flow
   ```

## Best Practices

### During Development

1. Keep the container running between test runs:
   ```bash
   ./run_e2e_tests.sh --start-only
   # Run tests multiple times
   MYSQL_HOST=localhost cargo test --test e2e_integration_test
   # Clean up when done
   ./run_e2e_tests.sh --stop-only
   ```

2. Use verbose output for debugging:
   ```bash
   cargo test --test e2e_integration_test -- --nocapture
   ```

3. Run specific tests:
   ```bash
   cargo test --test e2e_integration_test test_e2e_error_recovery
   ```

### In CI/CD

1. Always clean up containers, even on failure
2. Use health checks before running tests
3. Set reasonable timeouts
4. Cache Docker images to speed up builds
5. Run E2E tests separately from unit tests

### Test Maintenance

1. Keep test data minimal but representative
2. Clean up test databases in each test
3. Use unique database names to avoid conflicts
4. Document any special setup requirements
5. Update tests when requirements change

## Debugging Failed Tests

### Enable Detailed Logging

```bash
RUST_LOG=debug cargo test --test e2e_integration_test -- --nocapture
```

### Inspect MySQL State

```bash
# Connect to MySQL
docker-compose -f docker-compose.test.yml exec mysql-test mysql -u root -ptestpass

# List databases
SHOW DATABASES;

# Inspect a test database
USE e2e_test_db;
SHOW TABLES;
SELECT * FROM users;
```

### Check Container Logs

```bash
docker-compose -f docker-compose.test.yml logs mysql-test
```

### Run Tests in Isolation

```bash
# Run a single test
cargo test --test e2e_integration_test test_e2e_complete_query_flow -- --nocapture

# Run with backtrace
RUST_BACKTRACE=1 cargo test --test e2e_integration_test
```

## Adding New E2E Tests

When adding new E2E tests:

1. Follow the existing test structure
2. Use helper functions for common operations
3. Clean up test data after each test
4. Use unique database/table names
5. Document the test purpose and requirements validated
6. Update this guide with the new test information

Example template:

```rust
#[tokio::test]
async fn test_e2e_new_feature() {
    // Skip if MySQL is not available
    if !is_mysql_available().await {
        eprintln!("Skipping test: MySQL not available");
        return;
    }

    // Setup
    let config = create_test_datasource_config("test-db");
    let mut pool_manager = ConnectionPoolManager::new(config.clone()).await
        .expect("Failed to create pool manager");

    // Create test database
    create_test_database(&mut pool_manager, "new_feature_db").await
        .expect("Failed to create test database");

    // Test logic here
    
    // Assertions
    assert!(result.is_ok());
    
    println!("✓ New feature test passed");
}
```

## Resources

- [MySQL Docker Image Documentation](https://hub.docker.com/_/mysql)
- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [SQLx Documentation](https://docs.rs/sqlx/)

## Support

For issues or questions:
1. Check this guide first
2. Review test output and logs
3. Check existing GitHub issues
4. Create a new issue with:
   - Test output
   - Container logs
   - Environment details
   - Steps to reproduce
