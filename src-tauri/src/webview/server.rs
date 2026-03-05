use super::{
    templates::{default_css, default_html, default_js},
    websocket::{self, WsBroadcast},
    WebViewSettings,
};
use axum::{
    extract::State,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct WebViewServer {
    pub settings: Arc<RwLock<WebViewSettings>>,
    pub broadcast_tx: WsBroadcast,
}

impl WebViewServer {
    pub fn new(settings: Arc<RwLock<WebViewSettings>>) -> Self {
        Self {
            settings,
            broadcast_tx: websocket::create_broadcast_channel(),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let settings = self.settings.read().await;
        let addr = format!("{}:{}", settings.bind_address, settings.port);
        drop(settings);

        let app = Router::new()
            .route("/", get(index))
            .route("/ws", get(websocket::websocket_handler))
            .with_state((self.broadcast_tx.clone(), self.settings.clone()));

        let socket_addr: SocketAddr = addr.parse()?;
        let listener = tokio::net::TcpListener::bind(socket_addr).await?;

        tracing::info!("WebView server started on {}", addr);

        axum::serve(listener, app).await?;
        Ok(())
    }

    pub async fn broadcast_text(&self, text: String) {
        websocket::broadcast_text(&self.broadcast_tx, text);
    }
}

fn render_html(settings: &WebViewSettings) -> String {
    let html = if settings.html_template.is_empty() {
        default_html()
    } else {
        settings.html_template.clone()
    };

    let css = if settings.css_style.is_empty() {
        default_css()
    } else {
        settings.css_style.clone()
    };

    let js = default_js().replace("{{SPEED}}", &settings.animation_speed.to_string());

    html.replace("{{CSS}}", &css)
        .replace("{{JS}}", &js)
}

async fn index(
    State((_broadcast_tx, settings)): State<(WsBroadcast, Arc<RwLock<WebViewSettings>>)>,
) -> String {
    let s = settings.read().await;
    render_html(&s)
}
