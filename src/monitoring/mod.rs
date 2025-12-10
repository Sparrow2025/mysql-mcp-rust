use crate::manager::DataSourceManager;
use crate::pool::ConnectionPoolManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Monitoring service that periodically logs connection pool statistics
pub struct MonitoringService {
    manager: Arc<DataSourceManager>,
    pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
    interval_secs: u64,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MonitoringService {
    /// Create a new monitoring service
    pub fn new(
        manager: Arc<DataSourceManager>,
        pool_managers: Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
        interval_secs: u64,
    ) -> Self {
        Self {
            manager,
            pool_managers,
            interval_secs,
            task_handle: None,
        }
    }

    /// Start the monitoring service
    /// This will spawn a background task that logs connection pool statistics periodically
    pub fn start(mut self) -> Self {
        let manager = self.manager.clone();
        let pool_managers = self.pool_managers.clone();
        let interval_secs = self.interval_secs;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            // Skip the first tick (immediate)
            interval.tick().await;

            loop {
                interval.tick().await;

                // Log connection pool statistics
                log_pool_statistics(&manager, &pool_managers).await;
            }
        });

        self.task_handle = Some(handle);
        tracing::info!(
            interval_secs = interval_secs,
            "Monitoring service started"
        );
        self
    }

    /// Stop the monitoring service
    pub fn stop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            tracing::info!("Monitoring service stopped");
        }
    }
}

impl Drop for MonitoringService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Log connection pool statistics for all data sources
async fn log_pool_statistics(
    manager: &Arc<DataSourceManager>,
    pool_managers: &Arc<RwLock<HashMap<String, ConnectionPoolManager>>>,
) {
    let pool_managers_guard = pool_managers.read().await;

    // Get all data source keys
    let keys = manager.keys();

    if keys.is_empty() {
        tracing::debug!("No data sources configured");
        return;
    }

    tracing::info!(
        datasource_count = keys.len(),
        "Connection pool statistics report"
    );

    for key in keys {
        // Get data source status
        let status = manager.get_status(&key).await;

        // Get pool manager for this data source
        if let Some(pool_manager) = pool_managers_guard.get(&key) {
            let stats = pool_manager.get_stats();

            if stats.is_empty() {
                tracing::info!(
                    datasource_key = %key,
                    status = ?status,
                    "No active connection pools"
                );
            } else {
                for stat in stats {
                    tracing::info!(
                        datasource_key = %key,
                        database = %stat.database,
                        status = ?status,
                        active_connections = stat.active_connections,
                        idle_connections = stat.idle_connections,
                        total_connections = stat.total_connections,
                        "Connection pool statistics"
                    );
                }
            }
        } else {
            tracing::info!(
                datasource_key = %key,
                status = ?status,
                "Pool manager not initialized"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DataSourceConfig, PoolConfig};

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
    async fn test_monitoring_service_creation() {
        let configs = vec![create_test_config("db1", "Database 1")];
        let manager = DataSourceManager::new(configs).await.unwrap();
        let manager = Arc::new(manager);
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));

        let service = MonitoringService::new(manager, pool_managers, 60);
        assert!(service.task_handle.is_none());
    }

    #[tokio::test]
    async fn test_monitoring_service_start_stop() {
        let configs = vec![create_test_config("db1", "Database 1")];
        let manager = DataSourceManager::new(configs).await.unwrap();
        let manager = Arc::new(manager);
        let pool_managers = Arc::new(RwLock::new(HashMap::new()));

        let mut service = MonitoringService::new(manager, pool_managers, 60).start();
        assert!(service.task_handle.is_some());

        service.stop();
        assert!(service.task_handle.is_none());
    }
}
