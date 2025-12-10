use crate::config::DataSourceConfig;
use crate::error::{McpError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Connection status for a data source
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConnectionStatus {
    Available,
    Unavailable,
}

/// Information about a data source (without credentials)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataSourceInfo {
    pub key: String,
    pub name: String,
    pub status: ConnectionStatus,
}

/// Manages data sources and their connection pools
pub struct DataSourceManager {
    sources: HashMap<String, Arc<DataSourceConfig>>,
    // Track connection status for each data source (wrapped in RwLock for concurrent access)
    status: Arc<RwLock<HashMap<String, ConnectionStatus>>>,
    // Handle for the background reconnection task
    reconnect_task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl std::fmt::Debug for DataSourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceManager")
            .field("sources", &self.sources.keys().collect::<Vec<_>>())
            .field("status", &"<RwLock>")
            .field(
                "reconnect_task_active",
                &self.reconnect_task_handle.is_some(),
            )
            .finish()
    }
}

impl DataSourceManager {
    /// Create a new data source manager
    pub async fn new(configs: Vec<DataSourceConfig>) -> Result<Self> {
        let mut sources = HashMap::new();
        let mut status_map = HashMap::new();

        // Validate that all keys are unique
        let mut seen_keys = std::collections::HashSet::new();
        for config in &configs {
            if !seen_keys.insert(&config.key) {
                return Err(McpError::ConfigurationError(format!(
                    "Duplicate data source key: {}",
                    config.key
                )));
            }
        }

        for config in configs {
            tracing::info!(
                key = %config.key,
                name = %config.name,
                host = %config.host,
                "Registering data source"
            );
            
            let key = config.key.clone();
            sources.insert(key.clone(), Arc::new(config));
            // Initially mark all data sources as available
            status_map.insert(key, ConnectionStatus::Available);
        }

        let status = Arc::new(RwLock::new(status_map));

        Ok(Self {
            sources,
            status,
            reconnect_task_handle: None,
        })
    }

    /// Start the background reconnection task
    /// This task will periodically check unavailable data sources and attempt to reconnect
    pub fn start_reconnection_task(mut self) -> Self {
        let status = self.status.clone();
        let sources = self.sources.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                let unavailable_keys: Vec<String> = {
                    let status_guard = status.read().await;
                    status_guard
                        .iter()
                        .filter(|(_, &s)| s == ConnectionStatus::Unavailable)
                        .map(|(k, _)| k.clone())
                        .collect()
                };

                if !unavailable_keys.is_empty() {
                    tracing::info!(
                        count = unavailable_keys.len(),
                        "Attempting to reconnect to unavailable data sources"
                    );

                    for key in unavailable_keys {
                        if let Some(config) = sources.get(&key) {
                            tracing::debug!(
                                key = %key,
                                host = %config.host,
                                "Attempting reconnection"
                            );

                            // Here we would attempt to create a test connection
                            // For now, we'll just log the attempt
                            // In a real implementation, this would try to establish a connection
                            // and update the status accordingly
                            
                            // Placeholder for actual connection test
                            // let connection_result = test_connection(config).await;
                            // if connection_result.is_ok() {
                            //     let mut status_guard = status.write().await;
                            //     status_guard.insert(key.clone(), ConnectionStatus::Available);
                            //     tracing::info!(key = %key, "Data source reconnected successfully");
                            // }
                        }
                    }
                }
            }
        });

        self.reconnect_task_handle = Some(handle);
        self
    }

    /// Stop the background reconnection task
    pub fn stop_reconnection_task(&mut self) {
        if let Some(handle) = self.reconnect_task_handle.take() {
            handle.abort();
            tracing::info!("Background reconnection task stopped");
        }
    }

    /// Get a data source configuration by key
    /// Returns None if the key doesn't exist
    pub fn get_source(&self, key: &str) -> Option<Arc<DataSourceConfig>> {
        self.sources.get(key).cloned()
    }

    /// Validate that a data source key exists
    /// Returns an error if the key is invalid
    pub fn validate_key(&self, key: &str) -> Result<()> {
        if self.sources.contains_key(key) {
            Ok(())
        } else {
            Err(McpError::InvalidDataSourceKey(key.to_string()))
        }
    }

    /// List all data sources (without exposing credentials)
    pub async fn list_sources(&self) -> Vec<DataSourceInfo> {
        let status_guard = self.status.read().await;
        self.sources
            .iter()
            .map(|(key, config)| {
                let status = status_guard
                    .get(key)
                    .cloned()
                    .unwrap_or(ConnectionStatus::Unavailable);
                DataSourceInfo {
                    key: key.clone(),
                    name: config.name.clone(),
                    status,
                }
            })
            .collect()
    }

    /// Get the status of a specific data source
    pub async fn get_status(&self, key: &str) -> Option<ConnectionStatus> {
        let status_guard = self.status.read().await;
        status_guard.get(key).cloned()
    }

    /// Update the status of a data source
    pub async fn set_status(&self, key: &str, status: ConnectionStatus) {
        if self.sources.contains_key(key) {
            let mut status_guard = self.status.write().await;
            status_guard.insert(key.to_string(), status);

            match status {
                ConnectionStatus::Available => {
                    tracing::info!(key = %key, "Data source marked as available");
                }
                ConnectionStatus::Unavailable => {
                    tracing::warn!(key = %key, "Data source marked as unavailable");
                }
            }
        }
    }

    /// Check if a data source is available
    pub async fn is_available(&self, key: &str) -> bool {
        let status_guard = self.status.read().await;
        status_guard
            .get(key)
            .map(|s| *s == ConnectionStatus::Available)
            .unwrap_or(false)
    }

    /// Get all data source keys
    pub fn keys(&self) -> Vec<String> {
        self.sources.keys().cloned().collect()
    }

    /// Get the number of registered data sources
    pub fn count(&self) -> usize {
        self.sources.len()
    }

    /// Check if a data source key has permission for query operations
    pub fn check_query_permission(&self, key: &str) -> Result<()> {
        let config = self.get_source(key)
            .ok_or_else(|| McpError::InvalidDataSourceKey(key.to_string()))?;
        
        if config.permission.allows_query() {
            Ok(())
        } else {
            Err(McpError::PermissionDenied(format!(
                "Data source '{}' does not have query permission",
                key
            )))
        }
    }

    /// Check if a data source key has permission for update operations (DML)
    pub fn check_update_permission(&self, key: &str) -> Result<()> {
        let config = self.get_source(key)
            .ok_or_else(|| McpError::InvalidDataSourceKey(key.to_string()))?;
        
        if config.permission.allows_update() {
            Ok(())
        } else {
            Err(McpError::PermissionDenied(format!(
                "Data source '{}' does not have update permission (current: {:?})",
                key, config.permission
            )))
        }
    }

    /// Check if a data source key has permission for DDL operations
    pub fn check_ddl_permission(&self, key: &str) -> Result<()> {
        let config = self.get_source(key)
            .ok_or_else(|| McpError::InvalidDataSourceKey(key.to_string()))?;
        
        if config.permission.allows_ddl() {
            Ok(())
        } else {
            Err(McpError::PermissionDenied(format!(
                "Data source '{}' does not have DDL permission (current: {:?})",
                key, config.permission
            )))
        }
    }

    /// Get the permission level for a data source
    pub fn get_permission(&self, key: &str) -> Option<crate::config::Permission> {
        self.get_source(key).map(|config| config.permission)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PoolConfig;

    fn create_test_config(key: &str, name: &str) -> DataSourceConfig {
        DataSourceConfig {
            key: key.to_string(),
            name: name.to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "test".to_string(),
            password: "test".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        }
    }

    #[tokio::test]
    async fn test_new_manager_with_valid_configs() {
        let configs = vec![
            create_test_config("db1", "Database 1"),
            create_test_config("db2", "Database 2"),
        ];

        let manager = DataSourceManager::new(configs).await.unwrap();
        assert_eq!(manager.count(), 2);
        assert!(manager.get_source("db1").is_some());
        assert!(manager.get_source("db2").is_some());
    }

    #[tokio::test]
    async fn test_new_manager_with_duplicate_keys() {
        let configs = vec![
            create_test_config("db1", "Database 1"),
            create_test_config("db1", "Database 1 Duplicate"),
        ];

        let result = DataSourceManager::new(configs).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::ConfigurationError(_)));
    }

    #[tokio::test]
    async fn test_get_source() {
        let configs = vec![create_test_config("db1", "Database 1")];
        let manager = DataSourceManager::new(configs).await.unwrap();

        let source = manager.get_source("db1");
        assert!(source.is_some());
        assert_eq!(source.unwrap().key, "db1");

        let missing = manager.get_source("nonexistent");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_validate_key() {
        let configs = vec![create_test_config("db1", "Database 1")];
        let manager = DataSourceManager::new(configs).await.unwrap();

        assert!(manager.validate_key("db1").is_ok());
        assert!(manager.validate_key("nonexistent").is_err());
    }

    #[tokio::test]
    async fn test_list_sources_no_credentials() {
        let configs = vec![
            create_test_config("db1", "Database 1"),
            create_test_config("db2", "Database 2"),
        ];
        let manager = DataSourceManager::new(configs).await.unwrap();

        let sources = manager.list_sources().await;
        assert_eq!(sources.len(), 2);

        // Verify that credentials are not exposed
        for source in sources {
            assert!(!source.key.is_empty());
            assert!(!source.name.is_empty());
            // DataSourceInfo doesn't have password field, so credentials are not exposed
        }
    }

    #[tokio::test]
    async fn test_status_management() {
        let configs = vec![create_test_config("db1", "Database 1")];
        let manager = DataSourceManager::new(configs).await.unwrap();

        // Initially available
        assert_eq!(
            manager.get_status("db1").await,
            Some(ConnectionStatus::Available)
        );
        assert!(manager.is_available("db1").await);

        // Mark as unavailable
        manager
            .set_status("db1", ConnectionStatus::Unavailable)
            .await;
        assert_eq!(
            manager.get_status("db1").await,
            Some(ConnectionStatus::Unavailable)
        );
        assert!(!manager.is_available("db1").await);

        // Mark as available again
        manager.set_status("db1", ConnectionStatus::Available).await;
        assert_eq!(
            manager.get_status("db1").await,
            Some(ConnectionStatus::Available)
        );
        assert!(manager.is_available("db1").await);
    }

    #[tokio::test]
    async fn test_keys() {
        let configs = vec![
            create_test_config("db1", "Database 1"),
            create_test_config("db2", "Database 2"),
        ];
        let manager = DataSourceManager::new(configs).await.unwrap();

        let keys = manager.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"db1".to_string()));
        assert!(keys.contains(&"db2".to_string()));
    }
}
