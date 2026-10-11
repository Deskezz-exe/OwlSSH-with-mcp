//! The MCP server: streamable HTTP on 127.0.0.1, protected by a bearer token.

pub mod audit;
pub mod guard;
mod tools;

use std::sync::{Arc, Mutex};

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use serde::Serialize;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::settings::McpSettings;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub running: bool,
    pub port: u16,
    pub url: String,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct McpServer {
    stop: Mutex<Option<CancellationToken>>,
    status: Arc<Mutex<McpStatus>>,
}

impl McpServer {
    pub fn status(&self) -> McpStatus {
        self.status.lock().unwrap().clone()
    }

    /// Stops the running server (if any) and starts it again with `settings`.
    pub fn restart(&self, app: &AppHandle, settings: &McpSettings) {
        if let Some(token) = self.stop.lock().unwrap().take() {
            token.cancel();
        }
        *self.status.lock().unwrap() = McpStatus {
            port: settings.port,
            ..Default::default()
        };
        if !settings.enabled {
            return;
        }

        let token = CancellationToken::new();
        *self.stop.lock().unwrap() = Some(token.clone());
        let app = app.clone();
        let port = settings.port;
        let secret = settings.token.clone();
        let status = self.status.clone();
        tauri::async_runtime::spawn(async move {
            let result = serve(app, port, secret, token, status.clone()).await;
            let mut s = status.lock().unwrap();
            s.running = false;
            if let Err(e) = result {
                tracing::error!("MCP server stopped: {e}");
                s.error = Some(e.to_string());
            }
        });
    }
}

async fn serve(
    app: AppHandle,
    port: u16,
    secret: String,
    stop: CancellationToken,
    status: Arc<Mutex<McpStatus>>,
) -> std::io::Result<()> {
    let mut config = StreamableHttpServerConfig::default();
    config.cancellation_token = stop.child_token();
    let service = StreamableHttpService::new(
        move || Ok(tools::OwlTools::new(app.clone())),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let expected = format!("Bearer {secret}");
    let router =
        axum::Router::new()
            .nest_service("/mcp", service)
            .layer(axum::middleware::from_fn(
                move |req: Request, next: Next| {
                    let expected = expected.clone();
                    async move { require_token(req, next, &expected).await }
                },
            ));

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    {
        let mut s = status.lock().unwrap();
        s.running = true;
        s.url = format!("http://127.0.0.1:{port}/mcp");
        s.error = None;
    }
    axum::serve(listener, router)
        .with_graceful_shutdown(async move { stop.cancelled_owned().await })
        .await
}

async fn require_token(req: Request, next: Next, expected: &str) -> Result<Response, StatusCode> {
    let given = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if constant_time_eq(given.as_bytes(), expected.as_bytes()) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A random 32-character token for a fresh install.
pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_tokens() {
        assert!(constant_time_eq(b"Bearer abc", b"Bearer abc"));
        assert!(!constant_time_eq(b"Bearer abd", b"Bearer abc"));
        assert!(!constant_time_eq(b"Bearer ab", b"Bearer abc"));
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(new_token(), new_token());
        assert_eq!(new_token().len(), 32);
    }
}
