use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use lazy_static::lazy_static;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::WebViewSettings;

// Cached JSON format string for text messages (lazy_static for efficiency)
lazy_static! {
    static ref TEXT_MESSAGE_FORMAT: String = r#"{{"type":"text","data":"{}"}}"#.to_string();
}

/// Канал для broadcasting сообщений всем WebSocket клиентам
pub type WsBroadcast = broadcast::Sender<String>;

/// Обработчик WebSocket_upgrade
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((broadcast_tx, _settings)): State<(WsBroadcast, Arc<RwLock<WebViewSettings>>)>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, broadcast_tx))
}

/// Обработка WebSocket соединения
async fn handle_socket(socket: WebSocket, broadcast_tx: WsBroadcast) {
    let (mut sender, mut receiver) = socket.split();

    // Подписаться на broadcast канал
    let mut rx = broadcast_tx.subscribe();

    // Task для получения сообщений от клиента (если нужно в будущем)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Task для отправки сообщений клиенту
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Дождаться завершения любой из задач
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }
}

/// Создаёт broadcast канал для WebSocket сообщений
pub fn create_broadcast_channel() -> WsBroadcast {
    broadcast::channel(100).0
}

/// Отправляет текст всем подключённым клиентам
/// Uses cached format string for efficiency
pub fn broadcast_text(broadcast_tx: &WsBroadcast, text: String) {
    // Use cached format string instead of creating new one each time
    let json_text = TEXT_MESSAGE_FORMAT.replace("{}", &json_escape(&text));
    let _ = broadcast_tx.send(json_text);
}

/// Escape JSON string special characters
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
