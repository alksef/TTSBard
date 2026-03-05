# WebView Source Design

**Дата:** 2025-03-06
**Статус:** ✅ Утверждён

## Обзор

**Название фичи:** Animated Text Display Server (внутреннее название: `webview-source`)

**Назначение:** HTTP-сервер который отдаёт веб-страницу с анимированным текстом для использования в OBS Studio как Browser Source.

**Краткое описание:** При отправке текста на TTS, сервер через WebSocket отправляет его на все подключённые клиенты. Текст отображается с эффектом печатной машинки. Пользователь имеет полный контроль над HTML шаблоном и CSS стилями.

**Сценарий использования:** Стример добавляет `http://192.168.1.100:10100` как Browser Source в OBS. Когда TTSBard произносит текст, он появляется в OBS с анимацией.

---

## Архитектура

```
┌─────────────────────────────────────────────────────────────────┐
│                         TTSBard App                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌───────────────┐         ┌─────────────────────────────────┐ │
│  │   TTS Core    │────────▶│   WebViewSource Module (NEW)    │ │
│  │               │         │                                 │ │
│  │  text sent    │         │  ┌──────────────┐               │ │
│  │  to TTS       │         │  │ HTTP Server  │◀── Clients   │ │
│  └───────────────┘         │  │  (axum)      │   (OBS, etc) │ │
│                            │  └──────┬───────┘               │ │
│                            │         │                        │ │
│                            │  ┌──────▼───────┐               │ │
│                            │  │  WebSocket   │               │ │
│                            │  │  Broadcaster │               │ │
│                            │  └──────────────┘               │ │
│                            └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Компоненты:**

1. **HTTP Server** (`axum`) — слушает на `0.0.0.0:10100`
2. **WebSocket Broadcaster** — рассылает текст всем подключённым клиентам
3. **Settings Panel** — UI для настройки HTML/CSS/порта
4. **Storage** — сохраняет шаблоны в `AppData\ttsbard\webview\`

**Поток данных:**
```
User Input → TTS → Event Channel → WebViewSource → WebSocket → OBS Browser Source
```

---

## Структура файлов

```
src-tauri/src/
├── webview/
│   ├── mod.rs              # Module entry point
│   ├── server.rs           # HTTP server setup (axum)
│   ├── websocket.rs        # WebSocket broadcasting logic
│   └── templates.rs        # Default HTML/CSS templates
│
├── state.rs                # Add: WebViewSettings
├── settings.rs             # Add: webview settings persistence
├── events.rs               # Add: TextSentToTts event
└── commands/
    └── webview.rs          # Tauri commands for UI

src-tauri/src/state.rs:
┌─────────────────────────────────────────────────────┐
│  pub struct WebViewSettings {                        │
│      pub enabled: bool,                              │
│      pub port: u16,                                  │
│      pub bind_address: String,  // "0.0.0.0"         │
│      pub html_template: String,                      │
│      pub css_style: String,                          │
│      pub animation_speed: u32,  // ms per char       │
│  }                                                   │
└─────────────────────────────────────────────────────┘

src/components/
└── WebViewPanel.vue         # New settings panel

AppData/ttsbard/
└── webview/
    ├── template.html        # User's HTML template
    └── style.css            # User's CSS styles
```

---

## WebSocket протокол

**Сообщение от сервера:**
```json
{
  "type": "text",
  "text": "Привет, это тест TTS",
  "timestamp": 1709876543000
}
```

**Подключение клиентов:**
- Клиент (OBS) подключается к `ws://<IP>:<PORT>/ws`
- Сервер отправляет HTML страницу при GET `/`
- HTML содержит JS который подключается к WebSocket

**JavaScript клиент (встроенный в HTML):**
```javascript
const ws = new WebSocket(`ws://${location.host}/ws`);
ws.onmessage = (msg) => {
  const data = JSON.parse(msg.data);
  typewriterEffect(data.text);
};
```

---

## UI панель настроек

```
┌─────────────────────────────────────────────────────────────────┐
│  WebView Source                                    [⊗]  □        │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Сервер                                            [x] Включен   │
│  ─────────────────────────────────────────────────────────────  │
│  Порт:        [10100        ]                                  │
│  Bind:        [0.0.0.0       ▼]  (все интерфейсы)              │
│                                                                   │
│  Ссылка для OBS:                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ http://192.168.1.100:10100                               │   │
│  └──────────────────────────────────────────────────────────┘   │
│  [📋 Копировать]  [🔄 Обновить IP]                               │
│                                                                   │
│  HTML шаблон                                                     │
│  ─────────────────────────────────────────────────────────────  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ <!DOCTYPE html>                                          │   │
│  │ <html>                                                   │   │
│  │   <head><style>{{CSS}}</style></head>                    │   │
│  │   <body>                                                 │   │
│  │     <div id="text-container"></div>                      │   │
│  │     <script>{{JS}}</script>                              │   │
│  │   </body>                                                 │   │
│  │ </html>                                                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│  [Сбросить на дефолт]                                            │
│                                                                   │
│  CSS стиль                                                        │
│  ─────────────────────────────────────────────────────────────  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ body { background: transparent; }                         │   │
│  │ #text-container {                                        │   │
│  │   font-family: 'Arial', sans-serif;                      │   │
│  │   font-size: 48px;                                       │   │
│  │   color: #ffffff;                                        │   │
│  │   text-shadow: 2px 2px 4px #000;                         │   │
│  │ }                                                         │   │
│  └──────────────────────────────────────────────────────────┘   │
│  [Сбросить на дефолт]                                            │
│                                                                   │
│  Скорость анимации:  [30    ] мс/символ                          │
│                                                                   │
│                              [Сохранить]                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Интеграция с TTS

Используется существующая event система в `src-tauri/src/events.rs`:

```rust
// Добавить новый event
pub enum AppEvent {
    TextSentToTts(String),  // НОВОЕ
    // существующие...
}

// src-tauri/src/webview/mod.rs
pub async fn start_webview_server(mut rx: Receiver<AppEvent>) {
    let server = AxumServer::bind(...);

    task::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let AppEvent::TextSentToTts(text) = event {
                broadcast_to_all_clients(text).await;
            }
        }
    });
}
```

Event отправляется из TTS модулей (`openai.rs`, `silero.rs`, `local.rs`) перед synthesizing.

---

## Обработка ошибок

| Ситуяция | Обработка |
|----------|-----------|
| Порт занят | Показать ошибку в UI, предложить другой порт |
| Нет подключённых клиентов | Игнорировать (не фатально) |
| Клиент отключился | Убрать из списка рассылки |
| Сервер останавливается | Закрыть все WebSocket соединения аккуратно |
| HTML/CSS невалидный | Показать предупреждение, но сохранить |
| IP меняется (DHCP) | Кнопка обновления IP в UI |

---

## Тестирование

Ручное тестирование:

1. **HTTP сервер:**
   - Запуск/остановка
   - Порт занят → ошибка в UI
   - Множественные подключения

2. **WebSocket:**
   - Подключение клиента
   - Отправка текста
   - Отключение клиента
   - Несколько клиентов одновременно

3. **UI:**
   - Сохранение настроек
   - Сброс на дефолт
   - Валидация порта (1024-65535)
   - Обновление IP

4. **Интеграция:**
   - Текст из TTS попадает в браузер
   - Все TTS провайдеры работают (OpenAI, Silero, Local)

5. **OBS:**
   - Добавить как Browser Source
   - Текст отображается с анимацией

---

## Зависимости

```toml
# src-tauri/Cargo.toml

[dependencies]
axum = "0.7"
tokio-tungstenite = "0.21"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

**Всего 4 новых зависимости.**

---

## Краткое резюме

**Фича в двух словах:**

HTTP-сервер на `axum` с WebSocket вещанием текста, отправленного на TTS. Пользователь настраивает HTML/CSS через UI, в OBS добавляется Browser Source. Текст отображается с эффектом печатной машинки. Порт 10100 по умолчанию, bind на 0.0.0.0 для доступа из локальной сети.

---

## Ответы на вопросы брейншторма

| Вопрос | Ответ |
|--------|-------|
| Цель | OBS/Стриминг |
| Тип анимации | Печатная машинка |
| Накопление текста | Только текущая фраза |
| Настройки стиля | Полный контроль HTML/CSS |
| Порт | Фиксированный (10100 по умолчанию, настраиваемый) |
| Доступ | Локальная сеть (0.0.0.0) |
| DNS | Без DNS, просто IP:порт |
| Интеграция | Через event систему |
