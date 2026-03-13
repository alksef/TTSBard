use super::{
    templates::{default_css, default_html},
    WebViewSettings,
};
use axum::{
    extract::State,
    http::header,
    response::{sse::Event, IntoResponse, Sse},
    routing::get,
    Router,
};
use futures::Stream;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

pub type SseSender = broadcast::Sender<String>;

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

    #[allow(dead_code)]
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
}

impl WebViewServer {
    pub async fn new(settings: Arc<RwLock<WebViewSettings>>) -> Result<Self, anyhow::Error> {
        let templates = TemplateCache::new().await?;
        Ok(Self {
            settings,
            sse_tx: broadcast::channel(100).0,
            templates,
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let settings = self.settings.read().await;
        let addr = format!("{}:{}", settings.bind_address, settings.port);
        drop(settings);

        let app = Router::new()
            .route("/", get(index))
            .route("/sse", get(sse_handler))
            .with_state((self.sse_tx.clone(), self.templates.clone()));

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
        axum::serve(listener, app).await?;
        Ok(())
    }

    pub async fn broadcast_text(&self, text: &str) {
        if let Err(e) = self.sse_tx.send(text.to_string()) {
            tracing::debug!(error = %e, "Failed to broadcast (no receivers)");
        }
    }
}

async fn sse_handler(
    State((sse_tx, _templates)): State<(SseSender, TemplateCache)>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = sse_tx.subscribe();

    let stream = futures::stream::unfold(rx, move |mut rx| async move {
        match rx.recv().await {
            Ok(text) => {
                let json = serde_json::json!({"text": text}).to_string();
                Some((Ok(Event::default().data(json)), rx))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(10))
    )
}

async fn index(
    State((_sse_tx, templates)): State<(SseSender, TemplateCache)>,
) -> impl IntoResponse {
    let rendered = templates.get_rendered().await;
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], rendered).into_response()
}
