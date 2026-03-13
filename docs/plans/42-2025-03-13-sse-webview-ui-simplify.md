# План #42: Миграция WebView Source на SSE + упрощение UI

**Дата:** 2025-03-13
**Номер:** 42
**Задачи:**
1. Заменить WebSocket на SSE
2. Упростить UI (убрать HTML/CSS/Animation блоки)
3. Добавить кнопку "Открыть папку шаблона"
4. Добавить тестовую отправку сообщений

---

## Почему SSE вместо WebSocket?

| Аспект | WebSocket | SSE |
|--------|-----------|-----|
| Сложность | Выше (handshake, framing) | Ниже (обычный HTTP) |
| Зависимости | tokio-tungstenite | Только axum |
| Авто-реконнект | Нужно реализовывать | Встроен в EventSource |
| Bidirectional | Да (не нужно) | Нет (достаточно) |
| Безопасность | WS protocol | Обычный HTTP |

---

## Фазы реализации

### Фаза 1: Backend - Миграция на SSE

**Файлы:**
- `src-tauri/src/webview/server.rs` - Заменить WebSocket на SSE
- `src-tauri/src/webview/websocket.rs` - Удалить/переписать
- `src-tauri/src/webview/templates.rs` - Упростить (убрать JS генерацию)
- `src-tauri/Cargo.toml` - Убрать `tokio-tungstenite`

**Изменения:**

1. **SSE Handler вместо WebSocket:**
```rust
use axum::response::{sse::{Event, Sse}, sse::KeepAlive};
use futures::stream::{self, Stream};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio::sync::broadcast;

// Типы для SSE
pub type SseSender = broadcast::Sender<String>;

pub struct WebViewServer {
    pub settings: Arc<RwLock<WebViewSettings>>,
    pub sse_tx: SseSender,
}

impl WebViewServer {
    pub fn new(settings: Arc<RwLock<WebViewSettings>>) -> Self {
        Self {
            settings,
            sse_tx: broadcast::channel(100).0,
        }
    }
}

// SSE endpoint
async fn sse_handler(
    State((sse_tx, settings)): State<(SseSender, Arc<RwLock<WebViewSettings>>)>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = sse_tx.subscribe();

    let stream = stream! {
        while let Ok(text) = rx.recv().await {
            let json = serde_json::json!({"text": text}).to_string();
            yield Ok(Event::default().data(json));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new())
}
```

2. **Новый дефолтный шаблон (Fade In/Out):**
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
    <script>
        const evtSource = new EventSource('/sse');
        const container = document.getElementById('text-container');
        let hideTimeout = null;

        evtSource.onmessage = (event) => {
            const data = JSON.parse(event.data);
            showText(data.text);
        };

        function showText(text) {
            // Сброс предыдущего таймаута
            if (hideTimeout) clearTimeout(hideTimeout);

            // Сброс анимации
            container.classList.remove('visible');
            void container.offsetWidth; // force reflow

            // Установить текст
            container.textContent = text;

            // Плавное появление
            requestAnimationFrame(() => {
                container.classList.add('visible');
            });

            // Исчезновение через 5 секунд
            hideTimeout = setTimeout(() => {
                container.classList.remove('visible');
            }, 5000);
        }
    </script>
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
    opacity: 0;
    transition: opacity 0.5s ease-in-out;
}

#text-container.visible {
    opacity: 1;
}"#.to_string()
}

// Удалить default_js() - JS теперь встроен в HTML
```

3. **Обновить маршруты:**
```rust
let app = Router::new()
    .route("/", get(index))
    .route("/sse", get(sse_handler))  // Заменить /ws на /sse
    .with_state((self.sse_tx.clone(), self.settings.clone()));
```

4. **Обновить broadcast:**
```rust
pub async fn broadcast_text(&self, text: &str) {
    let _ = self.sse_tx.send(text.to_string());
}
```

5. **Cargo.toml:**
```toml
# Удалить:
# tokio-tungstenite = "0.21"

# Изменить:
axum = { version = "0.7" }  # Убрать features = ["ws"]
```

---

### Фаза 2: Упрощение WebViewSettings

**Файлы:**
- `src-tauri/src/webview/mod.rs` - Обновить структуру
- `src-tauri/src/settings.rs` - Обновить загрузку/сохранение

**Изменения:**

```rust
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
    // Удалено: html_template, css_style, animation_speed
}
```

```rust
// load_webview_settings() - только server настройки
pub fn load_webview_settings() -> Result<WebViewSettings> {
    let config_dir = dirs::config_dir()
        .context("Failed to get config dir")?
        .join("ttsbard")
        .join("webview");

    let json_path = config_dir.join("settings.json");
    let html_path = config_dir.join("index.html");
    let css_path = config_dir.join("style.css");

    // Создать папку если не существует
    fs::create_dir_all(&config_dir)
        .context("Failed to create webview directory")?;

    // Создать дефолтные файлы если не существуют (первый запуск)
    if !html_path.exists() {
        eprintln!("[SETTINGS] Creating default HTML template");
        fs::write(&html_path, crate::webview::templates::default_html())
            .context("Failed to write default HTML")?;
    }

    if !css_path.exists() {
        eprintln!("[SETTINGS] Creating default CSS");
        fs::write(&css_path, crate::webview::templates::default_css())
            .context("Failed to write default CSS")?;
    }

    let (start_on_boot, port, bind_addr) = if json_path.exists() {
        // ... загрузка из JSON
    } else {
        (false, 10100, "::".to_string())
    };

    Ok(WebViewSettings {
        enabled: false,
        start_on_boot,
        port,
        bind_address: bind_addr,
    })
}

// save_webview_settings() - только server настройки
pub fn save_webview_settings(settings: &WebViewSettings) -> Result<()> {
    let server_settings = serde_json::json!({
        "start_on_boot": settings.start_on_boot,
        "port": settings.port,
        "bind_address": settings.bind_address,
    });
    // ...
}
```

---

### Фаза 3: Tauri Commands

**Файлы:**
- `src-tauri/src/commands/webview.rs` - Добавить команды

**Добавить команды:**

```rust
/// Открыть папку шаблона в проводнике
#[tauri::command]
async fn open_template_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;

    let config_dir = dirs::config_dir()
        .map_err(|e| e.to_string())?
        .join("ttsbard")
        .join("webview");

    fs::create_dir_all(&config_dir)
        .map_err(|e| e.to_string())?;

    let path = config_dir.to_str().ok_or("Invalid path")?;

    #[cfg(target_os = "windows")]
    app.shell().command("explorer").args([path]).spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    app.shell().command("open").args([path]).spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    app.shell().command("xdg-open").args([path]).spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Тестовая отправка текста в SSE
#[tauri::command]
async fn send_test_message(
    text: String,
    sse_tx: State<SseSender>,
) -> Result<(), String> {
    sse_tx.send(text).map_err(|e| e.to_string())?;
    Ok(())
}
```

---

### Фаза 4: Frontend - Упрощение UI

**Файлы:**
- `src/components/WebViewPanel.vue` - Упростить компонент

**Удалить блоки:**
- HTML шаблон (строки 333-349)
- CSS стиль (строки 351-364)
- Анимация (строки 366-385)

**Добавить блок "Шаблоны":**
```vue
<section class="settings-section">
  <h2>Шаблоны</h2>
  <p class="setting-hint">
    Файлы шаблонов находятся в папке appdata. Вы можете редактировать их напрямую.
  </p>
  <div class="setting-row">
    <button @click="openTemplateFolder" class="action-button">
      Открыть папку шаблона
    </button>
  </div>
</section>
```

**Добавить блок "Тест":**
```vue
<section class="settings-section">
  <h2>Тест</h2>
  <div class="setting-row">
    <input
      type="text"
      v-model="testMessage"
      placeholder="Текст для отправки..."
      class="test-input"
      @keyup.enter="sendTest"
    />
    <button @click="sendTest" class="test-button" :disabled="!settings.enabled || !testMessage">
      Отправить
    </button>
  </div>
</section>
```

**Обновить script:**
```typescript
// Удалить из интерфейса
interface WebViewSettings {
  enabled: boolean
  start_on_boot: boolean
  port: number
  bind_address: string
}

// Добавить
const testMessage = ref('')

// Удалить функции
// - saveHtmlTemplate, saveCssStyle, saveAnimationSettings
// - resetHtml, resetCss
// - getDefaultHtml, getDefaultCss

// Добавить функции
async function openTemplateFolder() {
  try {
    await invoke('open_template_folder')
    showError('Папка шаблона открыта')
  } catch (e) {
    showError('Не удалось открыть папку: ' + (e as Error).message)
  }
}

async function sendTest() {
  if (!testMessage.value.trim()) return

  try {
    await invoke('send_test_message', { text: testMessage.value })
    showError('Сообщение отправлено!')
    testMessage.value = ''
  } catch (e) {
    showError('Ошибка отправки: ' + (e as Error).message)
  }
}
```

---

### Фаза 5: Обновить capabilities

**Файл:** `src-tauri/capabilities/default.json`

```json
{
  "permissions": [
    "core:default",
    "core:window:allow-center",
    "core:window:allow-hide",
    "core:window:allow-show",
    "opener:default",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister",
    "global-shortcut:allow-is-registered",
    "dialog:allow-open",
    "shell:allow-execute",
    "shell:allow-open"
  ]
}
```

---

## Механизм распространения шаблонов

**Проблема:** Как доставить дефолтные шаблоны пользователю?

**Решение:** Проверка при первом запуске

### Логика:

1. **При запуске приложения** → `load_webview_settings()`
2. **Проверить существование файлов:**
   - `%APPDATA%\ttsbard\webview\index.html`
   - `%APPDATA%\ttsbard\webview\style.css`
3. **Если файла нет** → создать из дефолтного шаблона
4. **Если файл есть** → использовать существующий

### Преимущества:

| Аспект | Описание |
|--------|----------|
| **Первый запуск** | Файлы создаются автоматически |
| **Обновление приложения** | Существующие файлы не перезаписываются |
| **Кастомизация** | Пользователь может редактировать файлы |
| **Ресет** | Удалить файлы → они создадутся заново |

### Файлы в appdata:

```
%APPDATA%\ttsbard\webview\
├── settings.json    # серверные настройки (port, bind_address, start_on_boot)
├── index.html       # HTML шаблон (создаётся при первом запуске)
└── style.css        # CSS стили (создаётся при первом запуске)
```

### Обновление дефолтных шаблонов в будущих версиях:

**Вариант 1:** Добавить версию шаблона
```rust
const TEMPLATE_VERSION: u32 = 1;

// В settings.json сохранить
"template_version": 1

// При загрузке сравнить версии
if saved_version < TEMPLATE_VERSION {
    // Обновить шаблоны
}
```

**Вариант 2:** Ничего не делать (проще)
- Пользователь сам удаляет файлы для ресета
- Или добавить кнопку "Сбросить шаблоны" в UI

---

## Кэширование и обновление шаблонов

**Решение:** Кэшировать в памяти + кнопка "Обновить шаблоны"

### Backend:

```rust
// src-tauri/src/webview/server.rs

use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct TemplateCache {
    html: Arc<RwLock<String>>,
    css: Arc<RwLock<String>>,
}

impl TemplateCache {
    pub async fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()?.join("ttsbard").join("webview");

        let html = tokio::fs::read_to_string(config_dir.join("index.html"))
            .await
            .unwrap_or_else(|_| default_html());

        let css = tokio::fs::read_to_string(config_dir.join("style.css"))
            .await
            .unwrap_or_else(|_| default_css());

        Ok(Self {
            html: Arc::new(RwLock::new(html)),
            css: Arc::new(RwLock::new(css)),
        })
    }

    pub async fn reload(&self) -> Result<()> {
        let config_dir = dirs::config_dir()?.join("ttsbard").join("webview");

        let html = tokio::fs::read_to_string(config_dir.join("index.html"))
            .await?;
        let css = tokio::fs::read_to_string(config_dir.join("style.css"))
            .await?;

        *self.html.write().await = html;
        *self.css.write().await = css;

        Ok(())
    }

    pub async fn get_rendered(&self) -> String {
        let html = self.html.read().await;
        let css = self.css.read().await;
        html.replace("{{CSS}}", &css)
    }
}

// В WebViewServer
pub struct WebViewServer {
    pub settings: Arc<RwLock<WebViewSettings>>,
    pub sse_tx: SseSender,
    pub templates: TemplateCache,
}

async fn index(State((sse_tx, settings, templates)): State<(...)>) -> impl IntoResponse {
    let rendered = templates.get_rendered().await;
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], rendered).into_response()
}
```

### Tauri Command:

```rust
// src-tauri/src/commands/webview.rs

#[tauri::command]
async fn reload_templates(
    templates: State<TemplateCache>,
) -> Result<String, String> {
    templates.reload()
        .await
        .map_err(|e| e.to_string())?;

    Ok("Шаблоны обновлены! Нажмите F5 в OBS для применения изменений.")
}
```

### Frontend:

```vue
<!-- Блок "Шаблоны" -->
<section class="settings-section">
  <h2>Шаблоны</h2>
  <p class="setting-hint">
    Файлы шаблонов находятся в папке appdata. Вы можете редактировать их напрямую.
  </p>
  <div class="setting-row">
    <button @click="openTemplateFolder" class="action-button">
      Открыть папку шаблона
    </button>
    <button @click="reloadTemplates" class="action-button secondary">
      Обновить шаблоны
    </button>
  </div>
</section>
```

```typescript
async function reloadTemplates() {
  try {
    const message = await invoke<string>('reload_templates')
    // Покажет как warning (жёлтый бабл) - "Нажмите F5 в OBS..."
    showError(message)
  } catch (e) {
    showError('Не удалось обновить шаблоны: ' + (e as Error).message)
  }
}
```

**Добавить CSS для warning:**
```css
.message-box.warning {
  background: rgba(255, 193, 7, 0.92);
  border: 1px solid rgba(255, 193, 7, 0.4);
  color: #4a3f00;
}
```

**Обновить условие классов:**
```vue
<div v-if="errorMessage" class="message-box" :class="{
  error: errorMessage.includes('Failed') || errorMessage.includes('Error') || errorMessage.includes('ошибка') || errorMessage.includes('Ошибка'),
  success: errorMessage.includes('запущен') || errorMessage.includes('перезапущен') || errorMessage.includes('сохранен') || errorMessage.includes('successful') || errorMessage.includes('Saved'),
  info: errorMessage.includes('Тест') || errorMessage.includes('Testing') || errorMessage.includes('остан'),
  warning: errorMessage.includes('F5') || errorMessage.includes('OBS') || errorMessage.includes('шаблон')
}">
```

### Преимущества:

| Аспект | Описание |
|--------|----------|
| **Производительность** | Из кэша, без чтения диска |
| **Контроль** | Пользователь решает когда обновлять |
| **Оповещение** | Warning напомнит про F5 в OBS |

### Пользовательский опыт:
1. Открыть папку шаблона
2. Редактировать `index.html` или `style.css`
3. Нажать "Обновить шаблоны" в UI
4. Увидеть сообщение "Нажмите F5 в OBS"
5. В OBS: F5 или закрыть/открыть Browser Source
6. Изменения применены ✅

---

## Итоговые изменения по файлам

### Удалить/переписать:
- `src-tauri/src/webview/websocket.rs` - заменить на SSE логику (или удалить)

### Изменить:
- `src-tauri/src/webview/mod.rs` - WebViewSettings структура (убрать html/css/animation)
- `src-tauri/src/webview/server.rs` - SSE + TemplateCache
- `src-tauri/src/webview/templates.rs` - новый default_html с fade in/out JS
- `src-tauri/src/commands/webview.rs` - добавить open_template_folder, send_test_message, reload_templates
- `src-tauri/src/settings.rs` - загрузка/сохранение + создание дефолтных файлов
- `src-tauri/Cargo.toml` - убрать tokio-tungstenite
- `src-tauri/capabilities/default.json` - добавить shell permissions
- `src/components/WebViewPanel.vue` - упростить UI + добавить кнопки

---

## Проверка

1. **Backend:**
   - SSE работает на `/sse`
   - Текст отправляется через broadcast
   - Настройки сохраняются без HTML/CSS

2. **Frontend:**
   - UI упрощён (3 блока удалены)
   - Кнопка "Открыть папку" работает
   - Тестовая отправка работает

3. **OBS:**
   - Browser Source подключается
   - Текст отображается при отправке
   - EventSource авто-реконнект

---

## Порядок реализации

1. Backend SSE (server.rs, templates.rs)
2. Настройки (mod.rs, settings.rs)
3. Commands (webview.rs)
4. Frontend UI (WebViewPanel.vue)
5. Capabilities (default.json)
6. Тестирование
