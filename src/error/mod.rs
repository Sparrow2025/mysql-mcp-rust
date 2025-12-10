use regex::Regex;
use std::sync::OnceLock;
use thiserror::Error;

/// Error types for the MCP server
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Invalid data source key: {0}")]
    InvalidDataSourceKey(String),

    #[error("Database not found: {0}")]
    DatabaseNotFound(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query timeout")]
    QueryTimeout,

    #[error("Query execution error: {0}")]
    QueryExecutionError(String),

    #[error("Authentication error")]
    AuthenticationError,

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Stream cancelled")]
    StreamCancelled,

    #[error("Invalid resource URI: {0}")]
    InvalidResourceUri(String),

    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("DDL statement not allowed")]
    DdlNotAllowed,

    #[error("Invalid statement: {0}")]
    InvalidStatement(String),

    #[error("Pool error: {0}")]
    PoolError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Data source unavailable: {0}")]
    DataSourceUnavailable(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

impl McpError {
    /// Sanitize error message to remove any sensitive information
    /// This removes passwords, connection strings, and other credentials
    pub fn sanitize(&self) -> String {
        let message = self.to_string();
        sanitize_error_message(&message)
    }

    /// Check if this error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            McpError::ConnectionFailed(_)
                | McpError::NetworkError(_)
                | McpError::PoolError(_)
                | McpError::DataSourceUnavailable(_)
        )
    }

    /// Check if this error is a connection-related error
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            McpError::ConnectionFailed(_) | McpError::NetworkError(_)
        )
    }
}

pub type Result<T> = std::result::Result<T, McpError>;

/// Patterns to match sensitive information in error messages
static SENSITIVE_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

fn get_sensitive_patterns() -> &'static Vec<Regex> {
    SENSITIVE_PATTERNS.get_or_init(|| {
        vec![
            // Password in connection strings (with or without spaces around =)
            Regex::new(r"password\s*[=:]\s*[^\s;]+").unwrap(),
            Regex::new(r"pwd\s*[=:]\s*[^\s;]+").unwrap(),
            // MySQL connection strings with protocol
            // Match mysql://user:pass@host where pass can contain special chars
            // We match up to the last @ before a hostname pattern
            Regex::new(r"mysql://[^:]+:.+?@[a-zA-Z0-9.-]+").unwrap(),
            // Generic credentials with Password/Pass prefix
            Regex::new(r"[Pp]assword:\s*\S+").unwrap(),
            Regex::new(r"[Pp]ass:\s*\S+").unwrap(),
            // API keys and tokens
            Regex::new(r"[Aa]pi[_-]?[Kk]ey\s*[=:]\s*\S+").unwrap(),
            Regex::new(r"[Tt]oken\s*[=:]\s*\S+").unwrap(),
            // Connection strings with embedded credentials (with protocol)
            // Match ://user:pass@host where pass can contain special chars
            Regex::new(r"://[^:]+:.+?@[a-zA-Z0-9.-]+").unwrap(),
            // Connection strings with embedded credentials (without protocol)
            // Matches patterns like user:pass@host or user:pass@host:port
            // Using non-greedy match for password to handle @ in passwords
            Regex::new(r"\b[a-zA-Z0-9_-]+:.+?@\S+").unwrap(),
        ]
    })
}

/// Sanitize an error message by removing sensitive information
pub fn sanitize_error_message(message: &str) -> String {
    let mut sanitized = message.to_string();

    for pattern in get_sensitive_patterns() {
        sanitized = pattern
            .replace_all(&sanitized, "[REDACTED]")
            .to_string();
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_password_in_connection_string() {
        let message = "Connection failed: mysql://user:secret123@localhost:3306/db";
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("secret123"));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn test_sanitize_password_field() {
        let message = "Error: password=mypassword123 is invalid";
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("mypassword123"));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn test_sanitize_multiple_credentials() {
        let message = "Failed to connect: mysql://admin:pass123@host1 and mysql://user:pass456@host2";
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("pass123"));
        assert!(!sanitized.contains("pass456"));
        assert!(!sanitized.contains("admin"));
        assert!(!sanitized.contains("user"));
    }

    #[test]
    fn test_no_sanitization_needed() {
        let message = "Connection timeout after 30 seconds";
        let sanitized = sanitize_error_message(message);
        assert_eq!(message, sanitized);
    }

    #[test]
    fn test_mcp_error_sanitize() {
        let error = McpError::ConnectionFailed(
            "Failed to connect to mysql://user:password@localhost:3306".to_string(),
        );
        let sanitized = error.sanitize();
        assert!(!sanitized.contains("password"));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn test_is_transient() {
        assert!(McpError::ConnectionFailed("test".to_string()).is_transient());
        assert!(McpError::NetworkError("test".to_string()).is_transient());
        assert!(!McpError::QueryTimeout.is_transient());
        assert!(!McpError::AuthenticationError.is_transient());
    }

    #[test]
    fn test_is_connection_error() {
        assert!(McpError::ConnectionFailed("test".to_string()).is_connection_error());
        assert!(McpError::NetworkError("test".to_string()).is_connection_error());
        assert!(!McpError::QueryTimeout.is_connection_error());
    }
}

/// Retry configuration for connection attempts
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: 1000,  // 1 second
            max_backoff_ms: 4000,       // 4 seconds
            backoff_multiplier: 2.0,    // Exponential backoff
        }
    }
}

impl RetryConfig {
    /// Calculate the backoff duration for a given attempt number (0-indexed)
    pub fn backoff_duration(&self, attempt: u32) -> std::time::Duration {
        let backoff_ms = (self.initial_backoff_ms as f64
            * self.backoff_multiplier.powi(attempt as i32))
        .min(self.max_backoff_ms as f64) as u64;

        std::time::Duration::from_millis(backoff_ms)
    }
}

/// Retry a fallible async operation with exponential backoff
pub async fn retry_with_backoff<F, Fut, T>(
    operation: F,
    config: &RetryConfig,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    tracing::info!(
                        attempt = attempt + 1,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                if !e.is_transient() {
                    // Don't retry non-transient errors
                    tracing::debug!(
                        error = %e.sanitize(),
                        "Non-transient error, not retrying"
                    );
                    return Err(e);
                }

                last_error = Some(e);

                if attempt + 1 < config.max_attempts {
                    let backoff = config.backoff_duration(attempt);
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = config.max_attempts,
                        backoff_ms = backoff.as_millis(),
                        error = %last_error.as_ref().unwrap().sanitize(),
                        "Operation failed, retrying after backoff"
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    // All attempts failed
    let error = last_error.unwrap();
    tracing::error!(
        max_attempts = config.max_attempts,
        error = %error.sanitize(),
        "Operation failed after all retry attempts"
    );
    Err(error)
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_config_backoff_duration() {
        let config = RetryConfig::default();

        // First attempt: 1 second
        assert_eq!(config.backoff_duration(0).as_millis(), 1000);

        // Second attempt: 2 seconds
        assert_eq!(config.backoff_duration(1).as_millis(), 2000);

        // Third attempt: 4 seconds (capped at max)
        assert_eq!(config.backoff_duration(2).as_millis(), 4000);

        // Fourth attempt: still 4 seconds (capped)
        assert_eq!(config.backoff_duration(3).as_millis(), 4000);
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let config = RetryConfig::default();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || async {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok::<_, McpError>(42)
            },
            &config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff_ms: 10, // Short backoff for testing
            max_backoff_ms: 100,
            backoff_multiplier: 2.0,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || async {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err(McpError::ConnectionFailed("Transient error".to_string()))
                } else {
                    Ok::<_, McpError>(42)
                }
            },
            &config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff_ms: 10,
            max_backoff_ms: 100,
            backoff_multiplier: 2.0,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || async {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(McpError::ConnectionFailed("Always fails".to_string()))
            },
            &config,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_does_not_retry_non_transient_errors() {
        let config = RetryConfig::default();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            || async {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(McpError::QueryTimeout) // Non-transient error
            },
            &config,
        )
        .await;

        assert!(result.is_err());
        // Should only try once for non-transient errors
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
