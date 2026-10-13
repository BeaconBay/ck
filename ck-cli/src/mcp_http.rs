//! MCP over Streamable HTTP (`ck --serve-http <addr>`), for a client on another host that
//! reaches ck through a tunnel or proxy rather than spawning it.
//!
//! Stateless and JSON-direct: each POST carries one JSON-RPC request and gets one JSON
//! response, so no session or `initialize` round trip is needed. Every request shares the
//! one [`CkMcpServer`], so its model, stats cache and pagination sessions persist across
//! calls. The path sandbox is the stdio server's (cwd plus `CK_MCP_ALLOWED_ROOTS`).
//!
//! When `CK_MCP_API_KEY_FILE` names a file, every request must carry
//! `Authorization: Bearer <its contents>`; without it the server is unauthenticated and
//! should listen on loopback only. rmcp's `Host` check (loopback names by default) guards
//! against DNS rebinding either way.

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_service::Service;
use tracing::{debug, info, warn};

use crate::mcp_server::CkMcpServer;

/// Environment variable naming a file whose contents are the bearer key clients must send.
pub const API_KEY_FILE_ENV: &str = "CK_MCP_API_KEY_FILE";

pub async fn run(server: CkMcpServer, addr: SocketAddr) -> Result<()> {
    let api_key: Option<Arc<[u8]>> = match std::env::var_os(API_KEY_FILE_ENV) {
        Some(path) => {
            let key = std::fs::read_to_string(&path).with_context(|| {
                format!("reading {API_KEY_FILE_ENV} {}", path.to_string_lossy())
            })?;
            let key = key.trim();
            anyhow::ensure!(!key.is_empty(), "{API_KEY_FILE_ENV} names an empty file");
            Some(Arc::from(format!("Bearer {key}").into_bytes()))
        }
        None => {
            warn!("{API_KEY_FILE_ENV} is unset: the HTTP MCP server accepts every request");
            None
        }
    };

    let config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true);
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    // The bound address, not the requested one, so `127.0.0.1:0` reports its port.
    info!(
        "ck MCP server listening on http://{}",
        listener.local_addr()?
    );

    loop {
        let (stream, peer) = listener.accept().await?;
        let service = service.clone();
        let api_key = api_key.clone();
        tokio::spawn(async move {
            let handler = hyper::service::service_fn(move |req: Request<Incoming>| {
                let mut service = service.clone();
                let authorized = api_key
                    .as_deref()
                    .is_none_or(|expected| bearer_matches(&req, expected));
                async move {
                    if !authorized {
                        return Ok::<_, Infallible>(
                            Response::builder()
                                .status(StatusCode::UNAUTHORIZED)
                                .header("WWW-Authenticate", "Bearer")
                                .body(Full::new(Bytes::from_static(b"Unauthorized")).boxed())
                                .expect("valid response"),
                        );
                    }
                    service.call(req).await
                }
            });
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), handler)
                .await
            {
                debug!("connection from {peer}: {e}");
            }
        });
    }
}

/// Constant-time comparison of the request's Authorization header with `Bearer <key>`.
fn bearer_matches<B>(req: &Request<B>, expected: &[u8]) -> bool {
    let Some(given) = req.headers().get(hyper::header::AUTHORIZATION) else {
        return false;
    };
    let given = given.as_bytes();
    given.len() == expected.len()
        && given
            .iter()
            .zip(expected)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(auth: Option<&str>) -> Request<()> {
        let mut b = Request::builder();
        if let Some(a) = auth {
            b = b.header("Authorization", a);
        }
        b.body(()).unwrap()
    }

    #[test]
    fn bearer_key_must_match_exactly() {
        let expected = b"Bearer s3cret";
        assert!(bearer_matches(&req(Some("Bearer s3cret")), expected));
        assert!(!bearer_matches(&req(Some("Bearer s3cres")), expected));
        assert!(!bearer_matches(&req(Some("Bearer s3cret2")), expected));
        assert!(!bearer_matches(&req(None), expected));
    }
}
