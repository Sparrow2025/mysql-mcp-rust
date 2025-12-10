use mysql_mcp_server::config::ServerConfig;
use std::env;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_load_config_from_toml_file() {
    // Create a temporary config file
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("test_config.toml");
    
    let config_content = r#"
query_timeout_secs = 30
stream_chunk_size = 1000

[[data_sources]]
key = "test-db"
name = "Test Database"
host = "localhost"
port = 3306
username = "testuser"
password = "testpass"
databases = []

[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Load the config
    let config = ServerConfig::from_toml_file(&config_path).expect("Failed to load config");
    
    // Verify the config
    assert_eq!(config.data_sources.len(), 1);
    assert_eq!(config.data_sources[0].key, "test-db");
    assert_eq!(config.data_sources[0].name, "Test Database");
    assert_eq!(config.data_sources[0].host, "localhost");
    assert_eq!(config.data_sources[0].port, 3306);
    assert_eq!(config.data_sources[0].username, "testuser");
    assert_eq!(config.data_sources[0].password, "testpass");
    
    // Clean up
    fs::remove_file(&config_path).ok();
}

#[test]
fn test_load_config_with_env_var() {
    // Set an environment variable
    env::set_var("TEST_DB_PASSWORD", "secret_password");
    
    // Create a temporary config file
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("test_config_env.toml");
    
    let config_content = r#"
query_timeout_secs = 30
stream_chunk_size = 1000

[[data_sources]]
key = "test-db"
name = "Test Database"
host = "localhost"
port = 3306
username = "testuser"
password = "$TEST_DB_PASSWORD"
databases = []

[data_sources.pool_config]
max_connections = 10
min_connections = 2
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 1800
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Load the config
    let config = ServerConfig::from_toml_file(&config_path).expect("Failed to load config");
    
    // Verify the password was loaded from env var
    assert_eq!(config.data_sources[0].password, "secret_password");
    
    // Clean up
    fs::remove_file(&config_path).ok();
    env::remove_var("TEST_DB_PASSWORD");
}

#[test]
fn test_load_config_missing_env_var() {
    // Make sure the env var doesn't exist
    env::remove_var("MISSING_PASSWORD");
    
    // Create a temporary config file
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("test_config_missing_env.toml");
    
    let config_content = r#"
query_timeout_secs = 30
stream_chunk_size = 1000

[[data_sources]]
key = "test-db"
name = "Test Database"
host = "localhost"
port = 3306
username = "testuser"
password = "$MISSING_PASSWORD"
databases = []
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Try to load the config - should fail
    let result = ServerConfig::from_toml_file(&config_path);
    assert!(result.is_err());
    
    // Clean up
    fs::remove_file(&config_path).ok();
}

#[test]
fn test_load_config_invalid_toml() {
    // Create a temporary config file with invalid TOML
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("test_config_invalid.toml");
    
    let config_content = r#"
this is not valid TOML
[[data_sources
key = "test-db"
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Try to load the config - should fail
    let result = ServerConfig::from_toml_file(&config_path);
    assert!(result.is_err());
    
    // Clean up
    fs::remove_file(&config_path).ok();
}

#[test]
fn test_auto_detect_file_format() {
    // Create a temporary config file
    let temp_dir = env::temp_dir();
    let config_path = temp_dir.join("test_config_auto.toml");
    
    let config_content = r#"
query_timeout_secs = 30
stream_chunk_size = 1000

[[data_sources]]
key = "test-db"
name = "Test Database"
host = "localhost"
port = 3306
username = "testuser"
password = "testpass"
databases = []
"#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Load the config using auto-detect
    let config = ServerConfig::from_file(&config_path).expect("Failed to load config");
    
    // Verify the config
    assert_eq!(config.data_sources.len(), 1);
    assert_eq!(config.data_sources[0].key, "test-db");
    
    // Clean up
    fs::remove_file(&config_path).ok();
}
