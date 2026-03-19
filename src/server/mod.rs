//! Server module for HTTP API and WebSocket
//!
//! Provides:
//! - REST API for agent management and messaging
//! - WebSocket for real-time streaming responses

mod api;
mod state;
mod websocket;

pub use api::create_router;
pub use state::ServerState;
pub use websocket::ws_handler;

use std::net::SocketAddr;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::Result;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,
    /// Port to bind to
    pub port: u16,
    /// Enable CORS for all origins
    pub cors_permissive: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_permissive: false,
        }
    }
}

impl ServerConfig {
    /// Create config for localhost
    pub fn localhost(port: u16) -> Self {
        Self {
            port,
            ..Default::default()
        }
    }

    /// Create config for all interfaces
    pub fn all_interfaces(port: u16) -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port,
            cors_permissive: true,
        }
    }

    /// Get socket address
    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("Invalid address")
    }
}

/// Start the Axon server
pub async fn start_server(config: ServerConfig, state: ServerState) -> Result<()> {
    let addr = config.addr();

    let mut app = create_router(state).layer(TraceLayer::new_for_http());

    if config.cors_permissive {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        app = app.layer(cors);
    }

    info!("Starting Axon server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| crate::error::AxonError::config(format!("Failed to bind: {}", e)))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| crate::error::AxonError::config(format!("Server error: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(!config.cors_permissive);
    }

    #[test]
    fn test_server_config_localhost() {
        let config = ServerConfig::localhost(8080);
        assert_eq!(config.port, 8080);
        assert_eq!(config.host, "127.0.0.1");
    }

    #[test]
    fn test_server_config_all_interfaces() {
        let config = ServerConfig::all_interfaces(9000);
        assert_eq!(config.host, "0.0.0.0");
        assert!(config.cors_permissive);
    }
}
