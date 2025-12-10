use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::path::Path;

/// Permission levels for data source access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    /// Read-only access (SELECT queries only)
    Query,
    /// Read and write access (SELECT, INSERT, UPDATE, DELETE)
    Update,
    /// Full access including DDL (CREATE, ALTER, DROP, etc.)
    Ddl,
}

impl Permission {
    /// Check if this permission allows query operations
    pub fn allows_query(&self) -> bool {
        matches!(self, Permission::Query | Permission::Update | Permission::Ddl)
    }

    /// Check if this permission allows update operations (DML)
    pub fn allows_update(&self) -> bool {
        matches!(self, Permission::Update | Permission::Ddl)
    }

    /// Check if this permission allows DDL operations
    pub fn allows_ddl(&self) -> bool {
        matches!(self, Permission::Ddl)
    }
}

impl Default for Permission {
    fn default() -> Self {
        Permission::Query
    }
}

/// Configuration for a single data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// Unique key to identify this data source
    pub key: String,
    /// Human-readable name for this data source
    pub name: String,
    /// MySQL host
    pub host: String,
    /// MySQL port
    pub port: u16,
    /// MySQL username
    pub username: String,
    /// MySQL password (loaded from env var or config)
    pub password: String,
    /// List of allowed databases (empty means all)
    #[serde(default)]
    pub databases: Vec<String>,
    /// Connection pool configuration
    #[serde(default)]
    pub pool_config: PoolConfig,
    /// Permission level for this data source
    #[serde(default)]
    pub permission: Permission,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,
    /// Idle timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Maximum connection lifetime in seconds
    #[serde(default = "default_max_lifetime")]
    pub max_lifetime_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connection_timeout_secs: default_connection_timeout(),
            idle_timeout_secs: default_idle_timeout(),
            max_lifetime_secs: default_max_lifetime(),
        }
    }
}

impl PoolConfig {
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_secs(self.connection_timeout_secs)
    }

    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_secs)
    }

    pub fn max_lifetime(&self) -> Duration {
        Duration::from_secs(self.max_lifetime_secs)
    }
}

fn default_max_connections() -> u32 {
    // Optimized: Increased from 10 to 15 for better concurrency
    // Balances resource usage with throughput
    15
}

fn default_min_connections() -> u32 {
    // Optimized: Increased from 2 to 3 to reduce cold-start latency
    // Keeps pool warmer without excessive resource usage
    3
}

fn default_connection_timeout() -> u64 {
    // Optimized: Reduced from 30 to 20 for faster failure detection
    // Prevents long waits on connection issues
    20
}

fn default_idle_timeout() -> u64 {
    // Optimized: Reduced from 300 to 240 for faster resource reclamation
    // Balances connection reuse with resource efficiency
    240
}

fn default_max_lifetime() -> u64 {
    // Optimized: Reduced from 1800 to 1500 to prevent stale connections
    // Ensures connections are refreshed more frequently
    1500
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// List of data sources
    pub data_sources: Vec<DataSourceConfig>,
    /// Query timeout in seconds
    #[serde(default = "default_query_timeout")]
    pub query_timeout_secs: u64,
    /// Stream chunk size (number of rows)
    #[serde(default = "default_stream_chunk_size")]
    pub stream_chunk_size: usize,
}

impl ServerConfig {
    pub fn query_timeout(&self) -> Duration {
        Duration::from_secs(self.query_timeout_secs)
    }
    
    /// Validate and filter data sources, keeping only valid ones
    /// Invalid data sources are logged and skipped
    pub fn validate_and_filter(&mut self) {
        let original_count = self.data_sources.len();
        
        // Filter out invalid data sources
        self.data_sources.retain(|ds| {
            match ds.validate() {
                Ok(_) => true,
                Err(e) => {
                    // Log error and skip this data source
                    eprintln!("Skipping invalid data source '{}': {}", ds.key, e);
                    false
                }
            }
        });
        
        let filtered_count = self.data_sources.len();
        if filtered_count < original_count {
            eprintln!(
                "Filtered out {} invalid data source(s), {} valid data source(s) remaining",
                original_count - filtered_count,
                filtered_count
            );
        }
    }
}

fn default_query_timeout() -> u64 {
    // Optimized: Kept at 30 seconds as a balanced default
    // Suitable for most OLTP queries while allowing some complexity
    30
}

fn default_stream_chunk_size() -> usize {
    // Optimized: Increased from 1000 to 1500 for better throughput
    // Reduces overhead while maintaining reasonable memory usage
    1500
}

impl ServerConfig {
    /// Load configuration from a TOML file
    pub fn from_toml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileReadError(path.display().to_string(), e.to_string()))?;
        
        let mut config: ServerConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!("TOML parse error: {}", e)))?;
        
        // Load passwords from environment variables if they start with $
        for ds in &mut config.data_sources {
            if ds.password.starts_with('$') {
                let env_var = &ds.password[1..];
                ds.password = std::env::var(env_var)
                    .map_err(|_| ConfigError::EnvVarNotFound(env_var.to_string()))?;
            }
        }
        
        // Validate the configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from a YAML file
    #[cfg(feature = "yaml")]
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileReadError(path.display().to_string(), e.to_string()))?;
        
        let mut config: ServerConfig = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!("YAML parse error: {}", e)))?;
        
        // Load passwords from environment variables if they start with $
        for ds in &mut config.data_sources {
            if ds.password.starts_with('$') {
                let env_var = &ds.password[1..];
                ds.password = std::env::var(env_var)
                    .map_err(|_| ConfigError::EnvVarNotFound(env_var.to_string()))?;
            }
        }
        
        // Validate the configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from a file (auto-detect format based on extension)
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let extension = path.extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ConfigError::UnsupportedFormat("No file extension".to_string()))?;
        
        match extension {
            "toml" => Self::from_toml_file(path),
            #[cfg(feature = "yaml")]
            "yaml" | "yml" => Self::from_yaml_file(path),
            _ => Err(ConfigError::UnsupportedFormat(extension.to_string())),
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.data_sources.is_empty() {
            return Err(ConfigError::ValidationError("No data sources configured".to_string()));
        }
        
        // Check for duplicate keys
        let mut keys = std::collections::HashSet::new();
        for ds in &self.data_sources {
            if !keys.insert(&ds.key) {
                return Err(ConfigError::ValidationError(
                    format!("Duplicate data source key: {}", ds.key)
                ));
            }
            
            // Validate data source
            ds.validate()?;
        }
        
        Ok(())
    }
}

impl DataSourceConfig {
    /// Validate a single data source configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate key
        if self.key.is_empty() {
            return Err(ConfigError::ValidationError("Data source key is required".to_string()));
        }
        
        // Validate host
        if self.host.is_empty() {
            return Err(ConfigError::ValidationError(
                format!("Data source '{}': host is required", self.key)
            ));
        }
        
        // Validate port
        if self.port == 0 {
            return Err(ConfigError::ValidationError(
                format!("Data source '{}': invalid port (must be > 0)", self.key)
            ));
        }
        
        // Validate username
        if self.username.is_empty() {
            return Err(ConfigError::ValidationError(
                format!("Data source '{}': username is required", self.key)
            ));
        }
        
        // Validate password
        if self.password.is_empty() {
            return Err(ConfigError::ValidationError(
                format!("Data source '{}': password is required", self.key)
            ));
        }
        
        // Validate pool config
        self.pool_config.validate(&self.key)?;
        
        Ok(())
    }
}

impl PoolConfig {
    /// Validate pool configuration
    pub fn validate(&self, datasource_key: &str) -> Result<(), ConfigError> {
        if self.max_connections == 0 {
            return Err(ConfigError::ValidationError(
                format!("Data source '{}': max_connections must be > 0", datasource_key)
            ));
        }
        
        if self.min_connections > self.max_connections {
            return Err(ConfigError::ValidationError(
                format!(
                    "Data source '{}': min_connections ({}) cannot be greater than max_connections ({})",
                    datasource_key, self.min_connections, self.max_connections
                )
            ));
        }
        
        Ok(())
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read file '{0}': {1}")]
    FileReadError(String, String),
    
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),
    
    #[error("Environment variable '{0}' not found")]
    EnvVarNotFound(String),
    
    #[error("Configuration validation error: {0}")]
    ValidationError(String),
    
    #[error("Unsupported configuration format: {0}")]
    UnsupportedFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pool_config_defaults() {
        let pool_config = PoolConfig::default();
        assert_eq!(pool_config.max_connections, 15); // Optimized from 10
        assert_eq!(pool_config.min_connections, 3);  // Optimized from 2
        assert_eq!(pool_config.connection_timeout_secs, 20); // Optimized from 30
        assert_eq!(pool_config.idle_timeout_secs, 240); // Optimized from 300
        assert_eq!(pool_config.max_lifetime_secs, 1500); // Optimized from 1800
    }
    
    #[test]
    fn test_pool_config_duration_conversion() {
        let pool_config = PoolConfig::default();
        assert_eq!(pool_config.connection_timeout(), Duration::from_secs(20)); // Optimized from 30
        assert_eq!(pool_config.idle_timeout(), Duration::from_secs(240)); // Optimized from 300
        assert_eq!(pool_config.max_lifetime(), Duration::from_secs(1500)); // Optimized from 1800
    }
    
    #[test]
    fn test_server_config_defaults() {
        let config = ServerConfig {
            data_sources: vec![],
            query_timeout_secs: default_query_timeout(),
            stream_chunk_size: default_stream_chunk_size(),
        };
        assert_eq!(config.query_timeout(), Duration::from_secs(30));
        assert_eq!(config.stream_chunk_size, 1500); // Optimized from 1000
    }
    
    #[test]
    fn test_datasource_validation_missing_key() {
        let ds = DataSourceConfig {
            key: "".to_string(),
            name: "Test".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        assert!(ds.validate().is_err());
    }
    
    #[test]
    fn test_datasource_validation_missing_host() {
        let ds = DataSourceConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            host: "".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        assert!(ds.validate().is_err());
    }
    
    #[test]
    fn test_datasource_validation_invalid_port() {
        let ds = DataSourceConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            host: "localhost".to_string(),
            port: 0,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        assert!(ds.validate().is_err());
    }
    
    #[test]
    fn test_datasource_validation_missing_username() {
        let ds = DataSourceConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        assert!(ds.validate().is_err());
    }
    
    #[test]
    fn test_datasource_validation_missing_password() {
        let ds = DataSourceConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        assert!(ds.validate().is_err());
    }
    
    #[test]
    fn test_datasource_validation_valid() {
        let ds = DataSourceConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        assert!(ds.validate().is_ok());
    }
    
    #[test]
    fn test_pool_config_validation_zero_max_connections() {
        let pool_config = PoolConfig {
            max_connections: 0,
            min_connections: 0,
            connection_timeout_secs: 30,
            idle_timeout_secs: 300,
            max_lifetime_secs: 1800,
        };
        
        assert!(pool_config.validate("test").is_err());
    }
    
    #[test]
    fn test_pool_config_validation_min_greater_than_max() {
        let pool_config = PoolConfig {
            max_connections: 5,
            min_connections: 10,
            connection_timeout_secs: 30,
            idle_timeout_secs: 300,
            max_lifetime_secs: 1800,
        };
        
        assert!(pool_config.validate("test").is_err());
    }
    
    #[test]
    fn test_pool_config_validation_valid() {
        let pool_config = PoolConfig::default();
        assert!(pool_config.validate("test").is_ok());
    }
    
    #[test]
    fn test_server_config_validation_no_datasources() {
        let config = ServerConfig {
            data_sources: vec![],
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_server_config_validation_duplicate_keys() {
        let ds1 = DataSourceConfig {
            key: "test".to_string(),
            name: "Test 1".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        let ds2 = DataSourceConfig {
            key: "test".to_string(),
            name: "Test 2".to_string(),
            host: "localhost".to_string(),
            port: 3307,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        let config = ServerConfig {
            data_sources: vec![ds1, ds2],
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_server_config_validation_valid() {
        let ds = DataSourceConfig {
            key: "test".to_string(),
            name: "Test".to_string(),
            host: "localhost".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            databases: vec![],
            pool_config: PoolConfig::default(),
            permission: crate::config::Permission::default(),
        };
        
        let config = ServerConfig {
            data_sources: vec![ds],
            query_timeout_secs: 30,
            stream_chunk_size: 1000,
        };
        
        assert!(config.validate().is_ok());
    }
}
