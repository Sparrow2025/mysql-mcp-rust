mod config;
mod error;
mod logging;
mod manager;
mod mcp_server;
mod monitoring;
mod pool;
mod resources;
mod tools;

use config::ServerConfig;
use logging::init_tracing;
use mcp_server::MySqlMcpServerHandler;
use rmcp::service::ServiceExt;
use rmcp::transport::stdio;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with sensitive information filtering
    init_tracing();

    // Generate a trace ID for this server session
    let trace_id = uuid::Uuid::new_v4();
    let _span = tracing::info_span!("server_session", trace_id = %trace_id).entered();

    tracing::info!("MySQL MCP Server starting...");

    // Load configuration from file or environment
    let config = load_config()?;

    tracing::info!(
        datasource_count = config.data_sources.len(),
        "Configuration loaded successfully"
    );

    // Create MCP server handler
    let handler = MySqlMcpServerHandler::new(config).await?;

    tracing::info!("MCP server handler initialized, starting stdio transport...");

    // Create stdio transport (stdin, stdout)
    let transport = stdio();

    // Setup signal handling for graceful shutdown
    let shutdown_signal = setup_signal_handlers();

    // Clone handler for cleanup
    let handler_for_cleanup = handler.clone();

    // Serve the handler with the transport
    let running_service = handler.serve(transport).await?;

    // Store the running service in an Arc for sharing with shutdown handler
    let running_service = Arc::new(RwLock::new(Some(running_service)));
    let running_service_clone = running_service.clone();

    // Wait for shutdown signal
    shutdown_signal.await.ok();
    
    tracing::info!("Shutdown signal received, initiating graceful shutdown...");

    // Take ownership of the running service and drop it to trigger shutdown
    {
        let mut service_guard = running_service_clone.write().await;
        if let Some(service) = service_guard.take() {
            drop(service);
            tracing::info!("MCP service stopped");
        }
    }

    tracing::info!("MySQL MCP Server shutting down...");

    // Perform cleanup - close connection pools and stop monitoring
    handler_for_cleanup.cleanup().await;

    tracing::info!("MySQL MCP Server shutdown complete");

    Ok(())
}

/// Load configuration from file or environment variables
fn load_config() -> anyhow::Result<ServerConfig> {
    // Try to load from config file first
    let config_path = std::env::var("MCP_CONFIG_PATH")
        .unwrap_or_else(|_| "config.toml".to_string());

    if std::path::Path::new(&config_path).exists() {
        tracing::info!(path = %config_path, "Loading configuration from file");
        let config = ServerConfig::from_file(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
        Ok(config)
    } else {
        anyhow::bail!(
            "Configuration file '{}' not found. Please create a configuration file or set MCP_CONFIG_PATH environment variable.",
            config_path
        );
    }
}

/// Setup signal handlers for graceful shutdown
/// Returns a future that completes when a shutdown signal is received
fn setup_signal_handlers() -> tokio::sync::oneshot::Receiver<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = signal(SignalKind::terminate())
                .expect("Failed to setup SIGTERM handler");
            let mut sigint = signal(SignalKind::interrupt())
                .expect("Failed to setup SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM signal");
                }
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT signal");
                }
            }
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, only handle Ctrl+C
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to setup Ctrl+C handler");
            tracing::info!("Received Ctrl+C signal");
        }

        // Send shutdown signal
        let _ = tx.send(());
    });

    rx
}




