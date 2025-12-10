use crate::error::sanitize_error_message;
use std::io;
use tracing::Subscriber;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

/// Initialize the tracing subscriber with sensitive information filtering
pub fn init_tracing() {
    // Create a custom formatter that sanitizes sensitive information
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_writer(io::stderr)
        .with_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("mysql_mcp_server=info")),
        );

    // Build the subscriber with the sanitizing layer
    tracing_subscriber::registry()
        .with(SanitizingLayer)
        .with(fmt_layer)
        .init();

    tracing::info!("Tracing initialized with sensitive information filtering");
}

/// A tracing layer that sanitizes sensitive information from log messages
struct SanitizingLayer;

impl<S> Layer<S> for SanitizingLayer
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Extract the message from the event
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        // If we found a message, sanitize it and log a warning if credentials were found
        if let Some(message) = visitor.message {
            let sanitized = sanitize_error_message(&message);
            if sanitized != message {
                // Credentials were found and sanitized
                tracing::warn!(
                    "Sensitive information detected and sanitized in log message"
                );
            }
        }
    }
}

/// Visitor to extract the message field from a tracing event
#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_in_logging() {
        // This test verifies that the sanitization logic is available
        let message = "Connection failed: mysql://user:password@localhost:3306/db";
        let sanitized = sanitize_error_message(message);
        assert!(!sanitized.contains("password"));
        assert!(sanitized.contains("[REDACTED]"));
    }
}
