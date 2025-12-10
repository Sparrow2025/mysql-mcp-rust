use crate::config::DataSourceConfig;
use crate::error::{McpError, Result};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::{MySql, Pool};
use std::collections::HashMap;
use std::str::FromStr;

/// Statistics for a connection pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub database: String,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub total_connections: usize,
}

/// Manages connection pools for a single data source
/// Each database gets its own connection pool
#[derive(Debug)]
pub struct ConnectionPoolManager {
    pools: HashMap<String, Pool<MySql>>,
    config: DataSourceConfig,
}

impl ConnectionPoolManager {
    /// Create a new connection pool manager for a data source
    pub async fn new(config: DataSourceConfig) -> Result<Self> {
        tracing::info!(
            key = %config.key,
            host = %config.host,
            port = config.port,
            "Creating connection pool manager"
        );

        Ok(Self {
            pools: HashMap::new(),
            config,
        })
    }

    /// Get or create a connection pool for a specific database
    pub async fn get_pool(&mut self, database: &str) -> Result<&Pool<MySql>> {
        // If pool already exists, return it
        if self.pools.contains_key(database) {
            return Ok(self.pools.get(database).unwrap());
        }

        // Create new pool
        tracing::info!(
            key = %self.config.key,
            database = %database,
            "Creating new connection pool"
        );

        let pool = self.create_pool(database).await?;
        self.pools.insert(database.to_string(), pool);

        Ok(self.pools.get(database).unwrap())
    }

    /// Create a new connection pool for a database
    async fn create_pool(&self, database: &str) -> Result<Pool<MySql>> {
        // Build connection string
        let connection_url = format!(
            "mysql://{}:{}@{}:{}/{}",
            self.config.username,
            self.config.password,
            self.config.host,
            self.config.port,
            database
        );

        // Parse connection options
        let connect_options = MySqlConnectOptions::from_str(&connection_url)
            .map_err(|e| McpError::ConnectionFailed(format!("Invalid connection URL: {}", e)))?;

        // Create pool with configured options
        let pool = MySqlPoolOptions::new()
            .max_connections(self.config.pool_config.max_connections)
            .min_connections(self.config.pool_config.min_connections)
            .acquire_timeout(self.config.pool_config.connection_timeout())
            .idle_timeout(Some(self.config.pool_config.idle_timeout()))
            .max_lifetime(Some(self.config.pool_config.max_lifetime()))
            .connect_with(connect_options)
            .await
            .map_err(|e| {
                McpError::ConnectionFailed(format!(
                    "Failed to create connection pool for database '{}': {}",
                    database, e
                ))
            })?;

        tracing::info!(
            key = %self.config.key,
            database = %database,
            max_connections = self.config.pool_config.max_connections,
            min_connections = self.config.pool_config.min_connections,
            "Connection pool created successfully"
        );

        Ok(pool)
    }

    /// Get a connection from the pool for a specific database
    pub async fn get_connection(
        &mut self,
        database: &str,
    ) -> Result<sqlx::pool::PoolConnection<MySql>> {
        let pool = self.get_pool(database).await?;

        pool.acquire()
            .await
            .map_err(|e| McpError::ConnectionFailed(format!("Failed to acquire connection: {}", e)))
    }

    /// Perform health check on all pools
    pub async fn health_check(&self) -> Result<()> {
        for (database, pool) in &self.pools {
            // Try to execute a simple query
            sqlx::query("SELECT 1")
                .execute(pool)
                .await
                .map_err(|e| {
                    McpError::ConnectionFailed(format!(
                        "Health check failed for database '{}': {}",
                        database, e
                    ))
                })?;

            tracing::debug!(
                key = %self.config.key,
                database = %database,
                "Health check passed"
            );
        }

        Ok(())
    }

    /// Get statistics for all connection pools
    pub fn get_stats(&self) -> Vec<PoolStats> {
        self.pools
            .iter()
            .map(|(database, pool)| {
                let size = pool.size() as usize;
                let idle = pool.num_idle() as usize;

                PoolStats {
                    database: database.clone(),
                    active_connections: size.saturating_sub(idle),
                    idle_connections: idle,
                    total_connections: size,
                }
            })
            .collect()
    }

    /// Get statistics for a specific database pool
    pub fn get_database_stats(&self, database: &str) -> Option<PoolStats> {
        self.pools.get(database).map(|pool| {
            let size = pool.size() as usize;
            let idle = pool.num_idle() as usize;

            PoolStats {
                database: database.to_string(),
                active_connections: size.saturating_sub(idle),
                idle_connections: idle,
                total_connections: size,
            }
        })
    }

    /// Close all connection pools
    pub async fn close_all(&self) {
        for (database, pool) in &self.pools {
            tracing::info!(
                key = %self.config.key,
                database = %database,
                "Closing connection pool"
            );
            pool.close().await;
        }
    }

    /// Get the list of databases with active pools
    pub fn active_databases(&self) -> Vec<String> {
        self.pools.keys().cloned().collect()
    }

    /// Check if a pool exists for a database
    pub fn has_pool(&self, database: &str) -> bool {
        self.pools.contains_key(database)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PoolConfig;

    fn create_test_config() -> DataSourceConfig {
        DataSourceConfig {
            key: "test".to_string(),
            name: "Test Database".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "test".to_string(),
            password: "test".to_string(),
            databases: vec![],
            pool_config: PoolConfig {
                max_connections: 5,
                min_connections: 1,
                connection_timeout_secs: 10,
                idle_timeout_secs: 300,
                max_lifetime_secs: 1800,
            },
            permission: crate::config::Permission::default(),
        }
    }

    #[tokio::test]
    async fn test_new_pool_manager() {
        let config = create_test_config();
        let manager = ConnectionPoolManager::new(config).await;
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.active_databases().len(), 0);
    }

    #[test]
    fn test_pool_stats() {
        let config = create_test_config();
        let manager = ConnectionPoolManager {
            pools: HashMap::new(),
            config,
        };

        let stats = manager.get_stats();
        assert_eq!(stats.len(), 0);
    }

    #[test]
    fn test_has_pool() {
        let config = create_test_config();
        let manager = ConnectionPoolManager {
            pools: HashMap::new(),
            config,
        };

        assert!(!manager.has_pool("test_db"));
    }

    #[test]
    fn test_active_databases() {
        let config = create_test_config();
        let manager = ConnectionPoolManager {
            pools: HashMap::new(),
            config,
        };

        let databases = manager.active_databases();
        assert_eq!(databases.len(), 0);
    }
}
