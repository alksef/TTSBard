use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::WebViewSettings;

/// Сообщение для отправки клиентам
#[derive(Debug, Clone, Serialize)]
struct TextMessage {
    #[serde(rename = "type")]
    msg_type: String,
    text: String,
    timestamp: u64,
}

impl TextMessage {
    fn new(text: String) -> Self {
        Self {
            msg_type: "text".to_string(),
            text,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
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
pub fn broadcast_text(broadcast_tx: &WsBroadcast, text: String) {
    let msg = TextMessage::new(text);
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = broadcast_tx.send(json);
    }
}
