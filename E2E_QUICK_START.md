# E2E Tests Quick Start

## TL;DR

```bash
# Run all E2E tests
./run_e2e_tests.sh
```

## Prerequisites

- Docker and Docker Compose installed
- Rust toolchain installed

## Common Commands

### Run All Tests
```bash
./run_e2e_tests.sh
```

### Development Mode
```bash
# Start MySQL (keep running)
./run_e2e_tests.sh --start-only

# Run tests (multiple times)
MYSQL_HOST=localhost cargo test --test e2e_integration_test

# Stop MySQL when done
./run_e2e_tests.sh --stop-only
```

### Run Specific Test
```bash
MYSQL_HOST=localhost cargo test --test e2e_integration_test test_e2e_complete_query_flow
```

### Debug Mode
```bash
RUST_LOG=debug cargo test --test e2e_integration_test -- --nocapture
```

## Test Coverage

✓ Complete query flow (SELECT, WHERE, aggregation)  
✓ Multi-datasource concurrent access  
✓ Error recovery scenarios  
✓ Streaming large result sets (2500+ rows)  
✓ DML operations (INSERT, UPDATE, DELETE)  
✓ Schema introspection (tables, columns, keys)  
✓ List operations (datasources, databases)  
✓ Connection pool statistics  
✓ SQL injection prevention  

## Troubleshooting

### Tests Skipped?
```bash
# Check MySQL is running
docker-compose -f docker-compose.test.yml ps

# Check MySQL health
docker-compose -f docker-compose.test.yml exec mysql-test mysqladmin ping -h localhost -u root -ptestpass
```

### Connection Issues?
```bash
# View MySQL logs
docker-compose -f docker-compose.test.yml logs mysql-test

# Restart container
docker-compose -f docker-compose.test.yml restart
```

### Clean Start?
```bash
# Remove everything and start fresh
docker-compose -f docker-compose.test.yml down -v
./run_e2e_tests.sh
```

## More Information

- Detailed guide: `docs/e2e-testing-guide.md`
- Test documentation: `tests/E2E_TEST_README.md`
- Implementation summary: `docs/task18-e2e-tests-implementation.md`

## CI/CD Integration

```yaml
# GitHub Actions example
- name: Run E2E Tests
  run: |
    docker-compose -f docker-compose.test.yml up -d
    timeout 60 bash -c 'until docker-compose -f docker-compose.test.yml exec -T mysql-test mysqladmin ping -h localhost -u root -ptestpass --silent; do sleep 2; done'
    MYSQL_HOST=localhost MYSQL_PORT=3306 MYSQL_USER=root MYSQL_PASSWORD=testpass cargo test --test e2e_integration_test -- --test-threads=1
    docker-compose -f docker-compose.test.yml down -v
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| MYSQL_HOST | localhost | MySQL hostname |
| MYSQL_PORT | 3306 | MySQL port |
| MYSQL_USER | root | MySQL username |
| MYSQL_PASSWORD | testpass | MySQL password |

## Need Help?

1. Check the troubleshooting section above
2. Read `docs/e2e-testing-guide.md`
3. Check container logs: `docker-compose -f docker-compose.test.yml logs`
4. Run with verbose output: `cargo test --test e2e_integration_test -- --nocapture`
