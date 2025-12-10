# Environment Variables Guide

## Overview

The MySQL MCP Server supports environment variables for both configuration and sensitive data management. This guide explains how to use environment variables effectively.

## Configuration Environment Variables

### MCP_CONFIG_PATH

Specifies the path to the configuration file.

**Default**: `config.toml` (in current directory)

**Usage**:
```bash
MCP_CONFIG_PATH=/path/to/config.toml mysql-mcp-server
```

**Example**:
```bash
# Use a configuration file in a different location
MCP_CONFIG_PATH=/etc/mysql-mcp/config.toml mysql-mcp-server

# Use a YAML configuration file
MCP_CONFIG_PATH=/etc/mysql-mcp/config.yaml mysql-mcp-server
```

## Password Environment Variables

For security, database passwords should be stored in environment variables rather than in configuration files.

### Syntax

In your configuration file, reference an environment variable by prefixing it with `$`:

```toml
[[data_sources]]
key = "prod-db"
name = "Production Database"
host = "localhost"
port = 3306
username = "root"
password = "$MYSQL_PASSWORD"  # References MYSQL_PASSWORD environment variable
```

### Setting Environment Variables

#### Linux/macOS

**Temporary (current session only)**:
```bash
export MYSQL_PASSWORD="your_secure_password"
export DEV_DB_PASSWORD="dev_password"
```

**Permanent (add to ~/.bashrc or ~/.zshrc)**:
```bash
echo 'export MYSQL_PASSWORD="your_secure_password"' >> ~/.bashrc
source ~/.bashrc
```

#### Windows

**Temporary (current session only)**:
```cmd
set MYSQL_PASSWORD=your_secure_password
set DEV_DB_PASSWORD=dev_password
```

**Permanent (system-wide)**:
```cmd
setx MYSQL_PASSWORD "your_secure_password"
setx DEV_DB_PASSWORD "dev_password"
```

### Environment Variable Naming Conventions

Follow these best practices for naming environment variables:

1. **Use Uppercase**: `MYSQL_PASSWORD`, not `mysql_password`
2. **Use Underscores**: `PROD_DB_PASSWORD`, not `prod-db-password`
3. **Be Descriptive**: Include the data source or purpose
4. **Be Consistent**: Use a consistent naming pattern across all data sources

**Examples**:
- `PROD_DB_PASSWORD` - Production database password
- `DEV_DB_PASSWORD` - Development database password
- `ANALYTICS_DB_PASSWORD` - Analytics database password
- `STAGING_DB_PASSWORD` - Staging database password

## Complete Example

### Configuration File (config.toml)

```toml
query_timeout_secs = 30
stream_chunk_size = 1000

[[data_sources]]
key = "prod-db"
name = "Production Database"
host = "prod.example.com"
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

### Environment Setup Script

Create a script to set all required environment variables:

**setup-env.sh** (Linux/macOS):
```bash
#!/bin/bash

# Production database
export PROD_DB_PASSWORD="prod_secure_password_here"

# Development database
export DEV_DB_PASSWORD="dev_password_here"

echo "Environment variables set successfully"
```

Make it executable and source it:
```bash
chmod +x setup-env.sh
source setup-env.sh
```

**setup-env.bat** (Windows):
```batch
@echo off

REM Production database
set PROD_DB_PASSWORD=prod_secure_password_here

REM Development database
set DEV_DB_PASSWORD=dev_password_here

echo Environment variables set successfully
```

Run it:
```cmd
setup-env.bat
```

### Using .env Files (Development Only)

For development, you can use a `.env` file with a tool like `direnv` or `dotenv`:

**.env**:
```bash
PROD_DB_PASSWORD=prod_password
DEV_DB_PASSWORD=dev_password
MCP_CONFIG_PATH=./config.toml
```

**Important**: Never commit `.env` files to version control. Add them to `.gitignore`:

```gitignore
.env
.env.local
.env.*.local
config.toml
```

## Docker Environment Variables

When running in Docker, pass environment variables using the `-e` flag or `--env-file`:

### Using -e Flag

```bash
docker run -e MYSQL_PASSWORD="password" \
           -e DEV_DB_PASSWORD="dev_password" \
           mysql-mcp-server
```

### Using --env-file

Create an `env.list` file:
```
MYSQL_PASSWORD=password
DEV_DB_PASSWORD=dev_password
```

Run with:
```bash
docker run --env-file env.list mysql-mcp-server
```

### Docker Compose

**docker-compose.yml**:
```yaml
version: '3.8'
services:
  mysql-mcp-server:
    image: mysql-mcp-server
    environment:
      - MYSQL_PASSWORD=${MYSQL_PASSWORD}
      - DEV_DB_PASSWORD=${DEV_DB_PASSWORD}
      - MCP_CONFIG_PATH=/config/config.toml
    volumes:
      - ./config.toml:/config/config.toml
```

## Kubernetes Secrets

For Kubernetes deployments, use Secrets to manage sensitive data:

### Create a Secret

```bash
kubectl create secret generic mysql-mcp-secrets \
  --from-literal=PROD_DB_PASSWORD='prod_password' \
  --from-literal=DEV_DB_PASSWORD='dev_password'
```

### Use in Deployment

**deployment.yaml**:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mysql-mcp-server
spec:
  template:
    spec:
      containers:
      - name: mysql-mcp-server
        image: mysql-mcp-server:latest
        env:
        - name: PROD_DB_PASSWORD
          valueFrom:
            secretKeyRef:
              name: mysql-mcp-secrets
              key: PROD_DB_PASSWORD
        - name: DEV_DB_PASSWORD
          valueFrom:
            secretKeyRef:
              name: mysql-mcp-secrets
              key: DEV_DB_PASSWORD
```

## Security Best Practices

1. **Never Commit Passwords**: Never commit passwords or `.env` files to version control
2. **Use Strong Passwords**: Use long, random passwords for production databases
3. **Rotate Regularly**: Rotate passwords regularly and update environment variables
4. **Limit Access**: Restrict who can view environment variables in production
5. **Use Secrets Management**: Consider using tools like HashiCorp Vault, AWS Secrets Manager, or Azure Key Vault
6. **Audit Access**: Log and audit access to environment variables
7. **Separate Environments**: Use different passwords for development, staging, and production

## Troubleshooting

### Environment Variable Not Found

**Error**:
```
Error: Environment variable 'MYSQL_PASSWORD' not found
```

**Solutions**:
1. Verify the variable is set:
   ```bash
   echo $MYSQL_PASSWORD  # Linux/macOS
   echo %MYSQL_PASSWORD%  # Windows
   ```

2. Check the variable name matches exactly (case-sensitive)

3. Ensure the variable is exported (Linux/macOS):
   ```bash
   export MYSQL_PASSWORD="password"
   ```

4. Verify the variable is set in the same shell session where you run the server

### Variable Not Persisting

**Problem**: Environment variable disappears after closing terminal

**Solution**: Add the export statement to your shell profile:
- Bash: `~/.bashrc` or `~/.bash_profile`
- Zsh: `~/.zshrc`
- Fish: `~/.config/fish/config.fish`

### Docker Container Can't Access Variables

**Problem**: Environment variables not available in Docker container

**Solution**: Pass variables explicitly:
```bash
docker run -e MYSQL_PASSWORD="$MYSQL_PASSWORD" mysql-mcp-server
```

Or use `--env-file`:
```bash
docker run --env-file .env mysql-mcp-server
```

## Verification

To verify environment variables are set correctly:

```bash
# List all environment variables
env | grep DB_PASSWORD

# Check specific variable
echo $MYSQL_PASSWORD

# Test configuration loading
mysql-mcp-server --help  # Should not error about missing variables
```

## Additional Resources

- [12-Factor App: Config](https://12factor.net/config)
- [OWASP: Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)
- [Docker Environment Variables](https://docs.docker.com/compose/environment-variables/)
- [Kubernetes Secrets](https://kubernetes.io/docs/concepts/configuration/secret/)
