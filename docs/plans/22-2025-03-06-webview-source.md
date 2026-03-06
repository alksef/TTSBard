# WebView Source Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** HTTP-сервер с WebSocket вещанием текста, отправленного на TTS, для отображения в OBS как Browser Source с анимацией печатной машинки.

**Architecture:** Tauri 2 app с Rust backend и Vue 3 frontend. Добавляется новый модуль `webview/` с HTTP сервером на axum, интеграция через существующую event систему. Пользователь настраивает HTML/CSS через Vue компонент.

**Tech Stack:** axum (HTTP server), tokio-tungstenite (WebSocket), serde (JSON), Vue 3, TypeScript

---

## Task 1: Добавить зависимости в Cargo.toml

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Добавить зависимости**

Откройте `src-tauri/Cargo.toml` и добавьте в `[dependencies]`:

```toml
axum = "0.7"
tokio-tungstenite = "0.21"
tower-http = { version = "0.5", features = ["fs", "cors"] }
```

**Step 2: Сохранить файл**

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "deps: add axum and websocket dependencies for webview feature"
```

---

## Task 2: Создать структуру модуля webview

**Files:**
- Create: `src-tauri/src/webview/mod.rs`
- Create: `src-tauri/src/webview/server.rs`
- Create: `src-tauri/src/webview/websocket.rs`
- Create: `src-tauri/src/webview/templates.rs`

**Step 1: Создать mod.rs**

```rust
mod server;
mod websocket;
mod templates;

pub use server::WebViewServer;
pub use templates::{default_html, default_css, default_js};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Настройки WebView Source
#[derive(Debug, Clone)]
pub struct WebViewSettings {
    pub enabled: bool,
    pub port: u16,
    pub bind_address: String,
    pub html_template: String,
    pub css_style: String,
    pub animation_speed: u32,
}

impl Default for WebViewSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 10100,
            bind_address: "0.0.0.0".to_string(),
            html_template: default_html(),
            css_style: default_css(),
            animation_speed: 30,
        }
    }
}
```

**Step 2: Создать templates.rs с дефолтными шаблонами**

```rust
pub fn default_html() -> String {
    r#"<!DOCTYPE html>
<html lang="ru">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>TTSBard WebView</title>
    <style>{{CSS}}</style>
</head>
<body>
    <div id="text-container"></div>
    <script>{{JS}}</script>
</body>
</html>"#.to_string()
}

pub fn default_css() -> String {
    r#"body {
    margin: 0;
    padding: 0;
    background: transparent;
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 100vh;
}

#text-container {
    font-family: 'Arial', sans-serif;
    font-size: 48px;
    color: #ffffff;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.8);
    text-align: center;
    padding: 20px;
}"#.to_string()
}

pub fn default_js() -> String {
    r#"const ws = new WebSocket(`ws://${location.host}/ws`);
let currentText = '';
let charIndex = 0;
let timeoutId = null;

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    if (data.type === 'text') {
        typeWriter(data.text);
    }
};

function typeWriter(text) {
    // Остановить текущую анимацию
    if (timeoutId) {
        clearTimeout(timeoutId);
    }

    currentText = text;
    charIndex = 0;

    const container = document.getElementById('text-container');
    container.textContent = '';

    function type() {
        if (charIndex < currentText.length) {
            container.textContent += currentText.charAt(charIndex);
            charIndex++;
            timeoutId = setTimeout(type, {{SPEED}});
        }
    }

    type();
}

ws.onclose = () => {
    console.log('WebSocket disconnected, attempting to reconnect...');
    setTimeout(() => {
        location.reload();
    }, 2000);
};

ws.onerror = (error) => {
    console.error('WebSocket error:', error);
};"#.to_string()
}
```

**Step 3: Создать заглушки для server.rs и websocket.rs**

```rust
// server.rs
use axum::{routing::get, Router};
use std::net::SocketAddr;

pub struct WebViewServer {
    pub settings: super::WebViewSettings,
}

impl WebViewServer {
    pub fn new(settings: super::WebViewSettings) -> Self {
        Self { settings }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/", get(index))
            .route("/ws", get(websocket_handler));

        let addr = format!("{}:{}", self.settings.bind_address, self.settings.port);
        let socket_addr: SocketAddr = addr.parse()?;
        let listener = tokio::net::TcpListener::bind(socket_addr).await?;

        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn index() -> &'static str {
    "TTSBard WebView Source"
}

async fn websocket_handler() -> &'static str {
    "WebSocket endpoint"
}
```

```rust
// websocket.rs
// Заглушка - будет реализована в следующих задачах
```

**Step 4: Зарегистрировать модуль в lib.rs**

Добавьте в `src-tauri/src/lib.rs`:

```rust
mod webview;
```

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/webview/
git commit -m "feat: add webview module structure with templates"
```

---

## Task 3: Добавить WebViewSettings в AppState

**Files:**
- Modify: `src-tauri/src/state.rs`

**Step 1: Добавить импорт и поле в AppState**

```rust
use crate::webview::WebViewSettings;

pub struct AppState {
    // существующие поля...

    // Добавить:
    pub webview_settings: Arc<RwLock<WebViewSettings>>,
}
```

**Step 2: Обновить создание AppState в lib.rs**

В функции `run()` найдите создание `AppState` и добавьте:

```rust
let app_state = Arc::new(AppState {
    // существующие инициализации...

    webview_settings: Arc::new(RwLock::new(WebViewSettings::default())),
});
```

**Step 3: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat: add webview_settings to AppState"
```

---

## Task 4: Добавить event для TTS текста

**Files:**
- Modify: `src-tauri/src/events.rs`

**Step 1: Добавить новый вариант в enum AppEvent**

```rust
#[derive(Debug, Clone)]
pub enum AppEvent {
    // существующие варианты...

    /// Текст отправлен на TTS - для WebView Source
    TextSentToTts(String),
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/events.rs
git commit -m "feat: add TextSentToTts event for webview integration"
```

---

## Task 5: Отправлять event из TTS модулей

**Files:**
- Modify: `src-tauri/src/tts/openai.rs`
- Modify: `src-tauri/src/tts/silero.rs`
- Modify: `src-tauri/src/tts/local.rs`

**Step 1: Добавить отправку event в openai.rs**

Найдите функцию `synthesize()` и перед вызовом API добавьте:

```rust
use crate::events::{AppEvent, EventSender};

// В функции synthesize, перед HTTP запросом:
if let Some(tx) = &self.event_tx {
    let _ = tx.send(AppEvent::TextSentToTts(text.clone()));
}
```

**Step 2: Добавить отправку event в silero.rs**

Аналогично openai.rs, найдите где отправляется текст в Telegram:

```rust
use crate::events::{AppEvent, EventSender};

// Перед отправкой в bot:
if let Some(tx) = &self.event_tx {
    let _ = tx.send(AppEvent::TextSentToTts(text.clone()));
}
```

**Step 3: Добавить отправку event в local.rs**

```rust
use crate::events::{AppEvent, EventSender};

// Перед HTTP запросом к local TTS:
if let Some(tx) = &self.event_tx {
    let _ = tx.send(AppEvent::TextSentToTts(text.clone()));
}
```

**Step 4: Commit**

```bash
git add src-tauri/src/tts/openai.rs src-tauri/src/tts/silero.rs src-tauri/src/tts/local.rs
git commit -m "feat: send TextSentToTts event from all TTS providers"
```

---

## Task 6: Реализовать WebSocket с broadcasting

**Files:**
- Modify: `src-tauri/src/webview/websocket.rs`
- Modify: `src-tauri/src/webview/server.rs`

**Step 1: Полностью переписать websocket.rs**

```rust
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
    State(broadcast_tx): State<WsBroadcast>,
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
```

**Step 2: Обновить server.rs для использования WebSocket**

```rust
use super::{websocket::{self, WsBroadcast}, WebViewSettings};
use axum::{
    extract::State,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

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
            .with_state(self.broadcast_tx.clone());

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

async fn index(State(broadcast_tx): State<WsBroadcast>) -> String {
    // Заглушка - будет заменена на реальный HTML в следующей задаче
    "TTSBard WebView Source - see documentation for OBS setup".to_string()
}
```

**Step 3: Commit**

```bash
git add src-tauri/src/webview/websocket.rs src-tauri/src/webview/server.rs
git commit -m "feat: implement WebSocket broadcasting for webview"
```

---

## Task 7: Рендерить HTML с подстановкой CSS и JS

**Files:**
- Modify: `src-tauri/src/webview/server.rs`

**Step 1: Добавить функцию рендеринга HTML**

```rust
use super::templates::{default_css, default_html, default_js};

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

    let js = default_js()
        .replace("{{SPEED}}", &settings.animation_speed.to_string());

    html.replace("{{CSS}}", &css)
        .replace("{{JS}}", &js)
}
```

**Step 2: Обновить handler для index**

```rust
async fn index(State(settings): State<Arc<RwLock<WebViewSettings>>>) -> String {
    let settings = settings.read().await;
    render_html(&settings)
}
```

**Step 3: Обновить Router для передачи settings**

```rust
// В методе start():
let app = Router::new()
    .route("/", get(index))
    .route("/ws", get(websocket::websocket_handler))
    .with_state((self.broadcast_tx.clone(), self.settings.clone()));
```

**Step 4: Обновить сигнатуры handlers**

```rust
async fn index(
    State((broadcast_tx, settings)): State<(WsBroadcast, Arc<RwLock<WebViewSettings>>)>,
) -> String {
    let s = settings.read().await;
    render_html(&s)
}

// И websocket_handler тоже обновить:
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((broadcast_tx, _settings)): State<(WsBroadcast, Arc<RwLock<WebViewSettings>>)>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket::handle_socket(socket, broadcast_tx))
}
```

**Step 5: Commit**

```bash
git add src-tauri/src/webview/server.rs
git commit -m "feat: render HTML with CSS and JS substitution"
```

---

## Task 8: Запуск сервера при старте приложения

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Добавить запуск WebView сервера в main**

Найдите место где создаётся и запускается event loop (после создания `event_tx`):

```rust
use crate::webview::WebViewServer;

// После создания event_tx, добавить:
let webview_settings = app_state.webview_settings.clone();
let webview_event_tx = event_tx.clone();

tokio::spawn(async move {
    let settings = webview_settings.read().await;
    if settings.enabled {
        let server = WebViewServer::new(webview_settings.clone());
        drop(settings);

        // Подписаться на события и запускать сервер
        let mut rx = event_tx.subscribe();

        // Запуск сервера
        let server_clone = server.clone();
        tokio::spawn(async move {
            let _ = server_clone.start().await;
        });

        // Обработка событий
        while let Ok(event) = rx.recv().await {
            match event {
                crate::events::AppEvent::TextSentToTts(text) => {
                    server.broadcast_text(text).await;
                }
                _ => {}
            }
        }
    }
});
```

**Step 2: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: start webview server on app startup"
```

---

## Task 9: Создать Tauri commands для управления настройками

**Files:**
- Create: `src-tauri/src/commands/webview.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Step 1: Создать commands/webview.rs**

```rust
use crate::state::AppState;
use crate::webview::WebViewSettings;
use tauri::State;

/// Получить текущие настройки WebView
#[tauri::command]
pub async fn get_webview_settings(
    state: State<'_, AppState>,
) -> Result<WebViewSettings, String> {
    let settings = state.webview_settings.read().await;
    Ok(settings.clone())
}

/// Сохранить настройки WebView
#[tauri::command]
pub async fn save_webview_settings(
    settings: WebViewSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut s = state.webview_settings.write().await;
    *s = settings;

    // TODO: Сохранить в файл (будет в следующей задаче)
    Ok(())
}

/// Получить текущий IP адрес для UI
#[tauri::command]
pub async fn get_local_ip() -> Result<String, String> {
    // Получить локальный IP адрес
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;

    let local_ip = socket.local_addr()?.ip().to_string();
    Ok(local_ip)
}
```

**Step 2: Зарегистрировать commands в mod.rs**

```rust
pub mod webview;

// В invoke_handler добавить:
.invoke_handler(tauri::generate_handler![
    // существующие commands...
    commands::webview::get_webview_settings,
    commands::webview::save_webview_settings,
    commands::webview::get_local_ip,
])
```

**Step 3: Commit**

```bash
git add src-tauri/src/commands/webview.rs src-tauri/src/commands/mod.rs
git commit -m "feat: add Tauri commands for webview settings"
```

---

## Task 10: Сохранение и загрузка настроек из файла

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/webview/mod.rs`

**Step 1: Добавить пути для WebView настроек**

```rust
use std::path::PathBuf;

impl AppSettings {
    pub fn webview_dir(&self) -> PathBuf {
        self.config_dir.join("webview")
    }

    pub fn template_html_path(&self) -> PathBuf {
        self.webview_dir().join("template.html")
    }

    pub fn style_css_path(&self) -> PathBuf {
        self.webview_dir().join("style.css")
    }
}
```

**Step 2: Добавить методы для загрузки/сохранения WebView**

```rust
impl AppSettings {
    // существующие методы...

    pub fn load_webview_settings(&self) -> std::io::Result<WebViewSettings> {
        use crate::webview::templates::{default_css, default_html};

        let html = if self.template_html_path().exists() {
            std::fs::read_to_string(self.template_html_path())?
        } else {
            default_html()
        };

        let css = if self.style_css_path().exists() {
            std::fs::read_to_string(self.style_css_path())?
        } else {
            default_css()
        };

        // Создать директорию если не существует
        std::fs::create_dir_all(self.webview_dir())?;

        Ok(WebViewSettings {
            enabled: false, // по умолчанию выключен
            port: 10100,
            bind_address: "0.0.0.0".to_string(),
            html_template: html,
            css_style: css,
            animation_speed: 30,
        })
    }

    pub fn save_webview_settings(&self, settings: &WebViewSettings) -> std::io::Result<()> {
        std::fs::write(self.template_html_path(), &settings.html_template)?;
        std::fs::write(self.style_css_path(), &settings.css_style)?;
        Ok(())
    }
}
```

**Step 3: Добавить Serialize/Deserialize для WebViewSettings**

```rust
// В webview/mod.rs:
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebViewSettings {
    pub enabled: bool,
    pub port: u16,
    pub bind_address: String,
    pub html_template: String,
    pub css_style: String,
    pub animation_speed: u32,
}
```

**Step 4: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/webview/mod.rs
git commit -m "feat: add webview settings persistence"
```

---

## Task 11: Создать Vue компонент WebViewPanel

**Files:**
- Create: `src/components/WebViewPanel.vue`

**Step 1: Создать компонент**

```vue
<template>
  <div class="panel">
    <h2>WebView Source</h2>

    <div class="section">
      <h3>Сервер</h3>
      <label class="checkbox">
        <input type="checkbox" v-model="settings.enabled" />
        Включен
      </label>

      <div class="field">
        <label>Порт:</label>
        <input
          type="number"
          v-model.number="settings.port"
          min="1024"
          max="65535"
        />
      </div>

      <div class="field">
        <label>Bind:</label>
        <select v-model="settings.bind_address">
          <option value="0.0.0.0">0.0.0.0 (все интерфейсы)</option>
          <option value="127.0.0.1">127.0.0.1 (только локально)</option>
        </select>
      </div>

      <div class="field">
        <label>Ссылка для OBS:</label>
        <div class="url-display">
          <code>{{ url }}</code>
          <button @click="copyUrl">📋</button>
          <button @click="refreshIp">🔄</button>
        </div>
      </div>
    </div>

    <div class="section">
      <h3>HTML шаблон</h3>
      <textarea
        v-model="settings.html_template"
        rows="15"
        spellcheck="false"
      ></textarea>
      <button @click="resetHtml">Сбросить на дефолт</button>
    </div>

    <div class="section">
      <h3>CSS стиль</h3>
      <textarea
        v-model="settings.css_style"
        rows="15"
        spellcheck="false"
      ></textarea>
      <button @click="resetCss">Сбросить на дефолт</button>
    </div>

    <div class="section">
      <h3>Скорость анимации</h3>
      <div class="field">
        <label>мс/символ:</label>
        <input
          type="number"
          v-model.number="settings.animation_speed"
          min="5"
          max="500"
        />
      </div>
    </div>

    <div class="actions">
      <button @click="save" class="primary">Сохранить</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/tauri';
import { readTextFile, writeTextFile } from '@tauri-apps/api/fs';
import { join } from '@tauri-apps/api/path';
import { documentDir } from '@tauri-apps/api/path';

interface WebViewSettings {
  enabled: boolean;
  port: number;
  bind_address: string;
  html_template: string;
  css_style: string;
  animation_speed: number;
}

const settings = ref<WebViewSettings>({
  enabled: false,
  port: 10100,
  bind_address: '0.0.0.0',
  html_template: '',
  css_style: '',
  animation_speed: 30,
});

const localIp = ref('192.168.1.100');

const url = computed(() => {
  return `http://${localIp.value}:${settings.value.port}`;
});

async function loadSettings() {
  try {
    const loaded = await invoke<WebViewSettings>('get_webview_settings');
    settings.value = loaded;
  } catch (e) {
    console.error('Failed to load settings:', e);
  }
}

async function save() {
  try {
    await invoke('save_webview_settings', { settings: settings.value });
    alert('Настройки сохранены!');
  } catch (e) {
    console.error('Failed to save settings:', e);
    alert('Ошибка сохранения: ' + e);
  }
}

async function refreshIp() {
  try {
    localIp.value = await invoke<string>('get_local_ip');
  } catch (e) {
    console.error('Failed to get local IP:', e);
  }
}

function copyUrl() {
  navigator.clipboard.writeText(url.value);
}

async function resetHtml() {
  if (confirm('Сбросить HTML шаблон на дефолтный?')) {
    settings.value.html_template = getDefaultHtml();
  }
}

async function resetCss() {
  if (confirm('Сбросить CSS стиль на дефолтный?')) {
    settings.value.css_style = getDefaultCss();
  }
}

function getDefaultHtml() {
  return `<!DOCTYPE html>
<html lang="ru">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>TTSBard WebView</title>
    <style>{{CSS}}</style>
</head>
<body>
    <div id="text-container"></div>
    <script>{{JS}}</script>
</body>
</html>`;
}

function getDefaultCss() {
  return `body {
    margin: 0;
    padding: 0;
    background: transparent;
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 100vh;
}

#text-container {
    font-family: 'Arial', sans-serif;
    font-size: 48px;
    color: #ffffff;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.8);
    text-align: center;
    padding: 20px;
}`;
}

onMounted(async () => {
  await loadSettings();
  await refreshIp();
});
</script>

<style scoped>
.panel {
  padding: 20px;
}

.section {
  margin-bottom: 30px;
  padding: 15px;
  background: var(--bg-secondary);
  border-radius: 8px;
}

h2 {
  margin-top: 0;
}

h3 {
  margin-top: 0;
  margin-bottom: 15px;
  font-size: 16px;
}

.field {
  margin: 10px 0;
  display: flex;
  align-items: center;
  gap: 10px;
}

.field label {
  min-width: 100px;
}

input[type="number"],
input[type="text"],
select {
  flex: 1;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg);
  color: var(--text);
}

.checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.url-display {
  display: flex;
  gap: 8px;
  align-items: center;
}

.url-display code {
  flex: 1;
  padding: 8px;
  background: var(--bg);
  border-radius: 4px;
  font-family: monospace;
}

textarea {
  width: 100%;
  font-family: monospace;
  font-size: 12px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg);
  color: var(--text);
  resize: vertical;
}

.actions {
  display: flex;
  justify-content: flex-end;
}

button {
  padding: 10px 20px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  background: var(--accent);
  color: white;
}

button:hover {
  opacity: 0.9;
}

button.primary {
  background: var(--primary);
}
</style>
```

**Step 2: Добавить в App.vue routing**

Найдите секцию с sidebar и добавьте:

```vue
<SidebarItem label="WebView Source" icon="🌐" />
```

И условный рендеринг:

```vue
<SidebarItem v-if="currentPanel === 'webview'" />
```

**Step 3: Commit**

```bash
git add src/components/WebViewPanel.vue src/App.vue
git commit -m "feat: add WebViewPanel settings UI"
```

---

## Task 12: Тестирование вручную

**Step 1: Запустить приложение**

```bash
cd src-tauri
cargo run
```

**Step 2: Открыть настройки WebView**

**Step 3: Включить сервер**

Поставить галочку "Включен", сохранить

**Step 4: Проверить в браузере**

Открыть `http://localhost:10100` - должна появиться страница

**Step 5: Проверить WebSocket**

Открыть DevTools Console, должна быть строка "WebSocket connected..."

**Step 6: Отправить текст на TTS**

Нажать глобальный хоткей, ввести текст, отправить

**Step 7: Проверить отображение**

Текст должен появиться с эффектом печатной машинки

**Step 8: Добавить в OBS**

1. Источник → Browser Source
2. URL: `http://localhost:10100`
3. Ширина/Высота: 1920x1080
4. Проверить отображение

---

## Полезные команды

```bash
# Сборка для релиза
cd src-tauri && cargo build --release

# Запуск с логами
RUST_LOG=debug cargo run

# Проверить порт
netstat -an | grep 10100  # Linux/Mac
netstat -an | findstr 10100  # Windows
```

---

## Checklist перед завершением

- [ ] Все тесты проходят
- [ ] Сервер запускается на указанном порту
- [ ] WebSocket соединение устанавливается
- [ ] Текст отправляется при TTS
- [ ] HTML/CSS настройки сохраняются
- [ ] Кнопка обновления IP работает
- [ ] OBS Browser Source показывает текст с анимацией
