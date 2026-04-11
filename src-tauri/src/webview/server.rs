use super::{
    templates::{default_css, default_html},
    WebViewSettings,
};
use crate::webview::security::{is_local_network, validate_token};
use super::upnp::UpnpManager;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{sse::Event, IntoResponse, Sse},
    routing::get,
    Router,
};
use futures::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

pub type SseSender = broadcast::Sender<String>;

const AUTH_COOKIE_NAME: &str = "webview_auth";

// Server state type
#[derive(Clone)]
pub struct ServerState {
    pub sse_tx: SseSender,
    pub templates: TemplateCache,
    pub access_token: Option<String>,
}

#[derive(Clone)]
pub struct TemplateCache {
    html: Arc<RwLock<String>>,
    css: Arc<RwLock<String>>,
    rendered: Arc<RwLock<String>>,
}

impl TemplateCache {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config dir"))?
            .join("ttsbard")
            .join("webview");

        // Ensure directory exists
        tokio::fs::create_dir_all(&config_dir)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create webview directory: {}", e))?;

        let html_path = config_dir.join("index.html");
        let css_path = config_dir.join("style.css");

        // Create default templates if they don't exist
        if !html_path.exists() {
            tracing::info!(html_path = ?html_path, "Creating default HTML template");
            tokio::fs::write(&html_path, default_html())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write default HTML: {}", e))?;
        }

        if !css_path.exists() {
            tracing::info!(css_path = ?css_path, "Creating default CSS");
            tokio::fs::write(&css_path, default_css())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write default CSS: {}", e))?;
        }

        let html = tokio::fs::read_to_string(&html_path)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, html_path = ?html_path, "Failed to read HTML, using default");
                default_html()
            });

        let css = tokio::fs::read_to_string(&css_path)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, css_path = ?css_path, "Failed to read CSS, using default");
                default_css()
            });

        let rendered = html.replace("{{CSS}}", &css);

        Ok(Self {
            html: Arc::new(RwLock::new(html)),
            css: Arc::new(RwLock::new(css)),
            rendered: Arc::new(RwLock::new(rendered)),
        })
    }

    pub async fn reload(&self) -> Result<(), anyhow::Error> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config dir"))?
            .join("ttsbard")
            .join("webview");

        let html_path = config_dir.join("index.html");
        let css_path = config_dir.join("style.css");

        let html = tokio::fs::read_to_string(&html_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read HTML: {}", e))?;

        let css = tokio::fs::read_to_string(&css_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read CSS: {}", e))?;

        let rendered = html.replace("{{CSS}}", &css);

        *self.html.write().await = html;
        *self.css.write().await = css;
        *self.rendered.write().await = rendered;

        Ok(())
    }

    pub async fn get_rendered(&self) -> String {
        self.rendered.read().await.clone()
    }
}

#[derive(Clone)]
pub struct WebViewServer {
    pub settings: Arc<RwLock<WebViewSettings>>,
    pub sse_tx: SseSender,
    pub templates: TemplateCache,
    pub upnp_manager: Option<Arc<UpnpManager>>,
}

impl WebViewServer {
    pub async fn new(settings: Arc<RwLock<WebViewSettings>>) -> Result<Self, anyhow::Error> {
        let templates = TemplateCache::new().await?;
        let s = settings.read().await;
        let port = s.port;
        drop(s);

        // Always create UPnP manager (will be toggled dynamically)
        tracing::info!("Creating UPnP manager for port {}", port);
        let upnp_manager = Some(Arc::new(UpnpManager::new(port)));

        Ok(Self {
            settings,
            sse_tx: broadcast::channel(100).0,
            templates,
            upnp_manager,
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let settings = self.settings.read().await;
        let addr = if settings.bind_address.contains(':') && !settings.bind_address.starts_with('[') {
            format!("[{}]:{}", settings.bind_address, settings.port)
        } else {
            format!("{}:{}", settings.bind_address, settings.port)
        };

        let access_token = settings.access_token.clone();
        let upnp_enabled = settings.upnp_enabled;
        drop(settings);

        // Forward UPnP port if enabled
        if upnp_enabled {
            if let Some(manager) = &self.upnp_manager {
                if let Err(e) = manager.forward() {
                    tracing::warn!(error = %e, "UPnP port forwarding failed, continuing anyway");
                }
            }
        }

        let state = ServerState {
            sse_tx: self.sse_tx.clone(),
            templates: self.templates.clone(),
            access_token,
        };

        let app = Router::new()
            .route("/", get(index))
            .route("/auth", get(auth_handler))
            .route("/sse", get(sse_handler))
            .with_state(state);

        let socket_addr: SocketAddr = addr.parse()
            .map_err(|e| format!("Invalid address {}: {}", addr, e))?;

        let listener = tokio::net::TcpListener::bind(socket_addr)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AddrInUse {
                    format!("Address {} is already in use.", addr)
                } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                    format!("Permission denied to bind to {}.", addr)
                } else {
                    format!("Failed to bind to {}: {}", addr, e)
                }
            })?;

        tracing::info!(addr = %addr, "WebView server started");
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
        Ok(())
    }

    pub async fn broadcast_text(&self, text: &str) {
        if let Err(e) = self.sse_tx.send(text.to_string()) {
            tracing::debug!(error = %e, "Failed to broadcast (no receivers)");
        }
    }

    /// Stop the server and clean up resources (including UPnP)
    pub fn stop(&self) {
        tracing::info!("Stopping WebViewServer and cleaning up resources");
        if let Some(manager) = &self.upnp_manager {
            tracing::info!("Removing UPnP port mapping on server stop");
            manager.remove();
        }
    }

    /// Toggle UPnP port forwarding dynamically without server restart
    pub fn toggle_upnp(&self, enabled: bool) {
        if let Some(manager) = &self.upnp_manager {
            if enabled {
                tracing::info!("Enabling UPnP port forwarding");
                if let Err(e) = manager.forward() {
                    tracing::warn!(error = %e, "Failed to enable UPnP port forwarding");
                }
            } else {
                tracing::info!("Disabling UPnP port forwarding");
                manager.remove();
            }
        }
    }
}

#[derive(Deserialize)]
struct AuthQuery {
    token: Option<String>,
}

// Helper function to extract cookie from headers
fn get_cookie_from_headers(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get("cookie")?.to_str().ok()?;
    cookie_header
        .split(';')
        .find_map(|pair| {
            let mut parts = pair.trim().splitn(2, '=');
            if parts.next()? == name {
                parts.next().map(|s| urlencoding::decode(s).unwrap_or(s.into()).into_owned())
            } else {
                None
            }
        })
}

async fn auth_handler(
    Query(params): Query<AuthQuery>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    if validate_token(params.token.as_deref(), state.access_token.as_deref()) {
        // Return Set-Cookie header with the token
        let cookie_value = state.access_token.as_ref()
            .map(|token| format!("{}={}; HttpOnly; Path=/; SameSite=Lax", AUTH_COOKIE_NAME, token))
            .unwrap_or_default();

        let mut response = (StatusCode::OK, "Авторизация успешна").into_response();
        if let Ok(cookie_header) = cookie_value.parse() {
            response.headers_mut().insert(header::SET_COOKIE, cookie_header);
        }
        response
    } else {
        (StatusCode::UNAUTHORIZED, "Неверный токен").into_response()
    }
}

async fn sse_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    // Check authentication
    let is_auth = if is_local_network(addr.ip()) {
        true
    } else {
        let cookie_token = get_cookie_from_headers(&headers, AUTH_COOKIE_NAME);
        validate_token(cookie_token.as_deref(), state.access_token.as_deref())
    };

    if !is_auth {
        tracing::warn!(addr = %addr.ip(), "Unauthorized SSE connection attempt");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let rx = state.sse_tx.subscribe();

    let stream = futures::stream::unfold(rx, move |mut rx| async move {
        match rx.recv().await {
            Ok(text) => {
                let json = serde_json::json!({"text": text}).to_string();
                Some((Ok(Event::default().data(json)), rx))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(10))
    ))
}

async fn index(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(params): Query<AuthQuery>,
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    // Check authentication
    let is_auth = if is_local_network(addr.ip()) {
        true
    } else {
        // First check cookie
        let cookie_token = get_cookie_from_headers(&headers, AUTH_COOKIE_NAME);
        let cookie_valid = validate_token(cookie_token.as_deref(), state.access_token.as_deref());

        // If cookie invalid, check query parameter token
        if cookie_valid {
            true
        } else {
            validate_token(params.token.as_deref(), state.access_token.as_deref())
        }
    };

    if !is_auth {
        tracing::warn!(addr = %addr.ip(), "Unauthorized page access attempt");
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // If token provided via query and is valid, set cookie for future requests
    let response = if params.token.is_some()
        && validate_token(params.token.as_deref(), state.access_token.as_deref())
    {
        let cookie_value = state.access_token.as_ref()
            .map(|token| format!("{}={}; HttpOnly; Path=/; SameSite=Lax", AUTH_COOKIE_NAME, token))
            .unwrap_or_default();

        let mut resp = ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], state.templates.get_rendered().await).into_response();
        if let Ok(cookie_header) = cookie_value.parse() {
            resp.headers_mut().insert(header::SET_COOKIE, cookie_header);
        }
        resp
    } else {
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], state.templates.get_rendered().await).into_response()
    };

    response
}
