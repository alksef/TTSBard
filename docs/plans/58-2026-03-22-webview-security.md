# План реализации безопасности WebView

## Обзор
Добавить UPnP для автоматического проброса портов и токен-аутентификацию для внешнего доступа к веб-вьюверу, сохраняя доступ из локальной сети без аутентификации.

## Выбор пользователя
- **Библиотека UPnP**: `easy-upnp` (подтверждён - лучший выбор для простоты)
- **Определение сети**: По диапазонам IP (локальные без токена)
- **Метод токена**: Query параметр устанавливает HttpOnly куку
- **Хранение токена**: В settings.json
- **Constant-time**: `subtle` crate для защиты от timing-атак
- **Cookie key**: Генерируется один раз, хранится в settings.json

---

## План реализации

### Этап 1: Зависимости

**Файл**: `src-tauri/Cargo.toml`

Добавить зависимости:
```toml
[dependencies]
# UPnP проброс портов - лучший выбор для простоты
easy-upnp = "0.3"

# Генерация токена и ключа кук
uuid = { version = "1.11", features = ["v4", "serde"] }

# Работа с куками (Axum 0.8.x совместимый)
tower-cookies = "0.10"
tower = "0.5"

# Constant-time сравнение (защита от timing-атак)
subtle = "2.6"
```

**Примечание по UPnP**: Альтернативы рассмотрены:
- `igd` - базовая библиотека (сложнее API)
- `portforwarder-rs` - НЕ тестируется на Windows/macOS
- `easy-upnp` - **рекомендуется**: простой API, кроссплатформенный, используется в rust-libp2p

### Этап 2: Структура настроек

**Файлы**:
- `src-tauri/src/webview/mod.rs`
- `src-tauri/src/config/settings.rs`

Добавить в `WebViewSettings`:
```rust
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
    // НОВЫЕ ПОЛЯ:
    pub access_token: Option<String>,
    pub upnp_enabled: bool,
    pub cookie_key: String,  // Ключ для подписи кук (генерируется один раз)
}
```

Добавить методы в SettingsManager:
- `set_webview_access_token()`
- `get_webview_access_token()`
- `set_webview_upnp_enabled()`
- `get_webview_upnp_enabled()`
- `set_webview_cookie_key()`
- `get_webview_cookie_key()`

### Этап 3: Модуль безопасности (НОВЫЙ)

**Файл**: `src-tauri/src/webview/security.rs`

```rust
use std::net::IpAddr;
use subtle::ConstantTimeEq;
use tower_cookies::Key;

/// Проверка: IP из локальной сети?
pub fn is_local_network(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) if addr.is_loopback() => true,     // 127.0.0.1
        IpAddr::V4(addr) if addr.is_private() => true,      // 192.168.x.x, 10.x.x.x, 172.16-31.x.x
        IpAddr::V4(addr) if addr.is_link_local() => true,   // 169.254.x.x
        IpAddr::V6(addr) if addr.is_loopback() => true,     // ::1
        IpAddr::V6(addr) => (addr.segments()[0] & 0xfe00) == 0xfc00,  // fc00::/7
        _ => false,
    }
}

/// Валидация токена (constant-time comparison)
pub fn validate_token(provided: Option<&str>, stored: Option<&str>) -> bool {
    match (provided, stored) {
        (Some(p), Some(s)) => {
            // Constant-time comparison через subtle crate
            let choice = p.ct_eq(s.as_bytes());
            bool::from(choice)
        }
        (None, None) => true,  // Токен не настроен - разрешить
        _ => false,
    }
}

/// Создать ключ для кук (генерируется один раз при первом запуске)
pub fn generate_cookie_key() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Парсинг ключа в tower-cookies Key
pub fn parse_cookie_key(key_str: &str) -> Result<Key, String> {
    let key_bytes: [u8; 32] = key_str
        .as_bytes()
        .get(..32)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| "Неверный формат ключа".to_string())?;
    Ok(Key::from(key_bytes))
}
```

### Этап 4: Модуль UPnP (НОВЫЙ)

**Файл**: `src-tauri/src/webview/upnp.rs`

```rust
use easy_upnp::{EasyUPnP, PortMappingProtocol};

pub struct UpnpManager {
    port: u16,
    upnp: Option<EasyUPnP>,
}

impl UpnpManager {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            upnp: EasyUPnP::new().ok(),
        }
    }

    /// Открыть порт на роутере
    pub fn forward(&self) -> Result<(), String> {
        let Some(upnp) = &self.upnp else {
            return Err("UPnP недоступен".to_string());
        };

        upnp.open_port(self.port, self.port, PortMappingProtocol::TCP)
            .map_err(|e| format!("Не открыть порт: {}", e))?;

        Ok(())
    }

    /// Закрыть порт на роутере
    pub fn remove(&self) {
        if let Some(upnp) = &self.upnp {
            let _ = upnp.close_port(self.port, PortMappingProtocol::TCP);
        }
    }
}

// Автоматическая очистка при drop
impl Drop for UpnpManager {
    fn drop(&mut self) {
        self.remove();
    }
}
```

### Этап 5: Обновление сервера

**Файл**: `src-tauri/src/webview/server.rs`

Добавить в `WebViewServer`:
```rust
pub struct WebViewServer {
    pub settings: Arc<RwLock<WebViewSettings>>,
    pub sse_tx: SseSender,
    pub templates: TemplateCache,
    pub upnp_manager: Option<Arc<UpnpManager>>,  // НОВОЕ
}
```

Добавить cookie layer (динамический ключ из настроек):
```rust
use tower_cookies::{CookieManagerLayer, Cookies, Cookie};
use tower_cookies::cookie::SameSite;

const AUTH_COOKIE_NAME: &str = "webview_auth";

// Получаем ключ из настроек
let cookie_key = parse_cookie_key(&settings.cookie_key)?;

let app = Router::new()
    .route("/", get(index))
    .route("/auth", get(auth_handler))  // НОВЫЙ
    .route("/sse", get(sse_handler))
    .layer(CookieManagerLayer::new(cookie_key))  // Динамический ключ
    .with_state((sse_tx, templates, access_token.clone()));
```

Новый auth handler:
```rust
use axum::extract::Query;
use serde::Deserialize;

#[derive(Deserialize)]
struct AuthQuery {
    token: Option<String>,
}

async fn auth_handler(
    Query(params): Query<AuthQuery>,
    State((_, _, access_token)): State<(SseSender, TemplateCache, Option<String>)>,
    cookies: Cookies,
) -> impl IntoResponse {
    if validate_token(params.token.as_deref(), access_token.as_deref()) {
        let token = access_token.unwrap();
        let mut cookie = Cookie::new(AUTH_COOKIE_NAME, token);
        cookie.set_http_only(true);
        cookie.set_same_site(SameSite::Lax);
        cookie.set_path("/");
        cookies.add(cookie);
        (StatusCode::OK, "Авторизация успешна")
    } else {
        (StatusCode::UNAUTHORIZED, "Неверный токен")
    }
}
```

Обновленный SSE handler:
```rust
async fn sse_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: Cookies,
    State((sse_tx, _, access_token)): State<(...)>,
) -> Result<impl IntoResponse, StatusCode> {
    // Локальная сеть без токена
    let is_auth = if is_local_network(addr.ip()) {
        true
    } else {
        // Внешняя сеть проверяем куку
        let cookie_token = cookies.get(AUTH_COOKIE_NAME).map(|c| c.value());
        validate_token(cookie_token, access_token.as_deref())
    };

    if !is_auth {
        return Err(StatusCode::UNAUTHORIZED);
    }
    // ... остальной handler
}
```

### Этап 6: Обновление шаблонов

**Файл**: `src-tauri/src/webview/templates.rs`

Обновить SSE подключение:
```javascript
const evtSource = new EventSource('/sse', {
    withCredentials: true  // Отправлять куки
});

evtSource.onerror = async (error) => {
    console.error('SSE error:', error);

    // Проверка на ошибку авторизации
    if (evtSource.readyState === EventSource.CLOSED) {
        const token = urlParams.get('token');
        if (token) {
            try {
                const resp = await fetch('/auth?token=' + encodeURIComponent(token), {
                    credentials: 'include'
                });
                // Только при успешном ответе перезагружаем
                if (resp.ok) {
                    window.location.reload();
                } else {
                    console.error('Авторизация не удалась');
                }
            } catch (e) {
                console.error('Ошибка авторизации:', e);
            }
        }
    }
};
```

### Этап 7: Tauri команды

**Файл**: `src-tauri/src/commands/webview.rs`

Добавить команды:
- `generate_webview_token()` — сгенерировать новый токен
- `get_webview_token()` — получить замаскированный токен
- `copy_webview_token()` — копировать токен в буфер обмена
- `regenerate_webview_token()` — перегенерировать токен
- `set_webview_upnp_enabled(enabled: bool)` — включить/выключить UPnP
- `get_webview_upnp_enabled()` — получить статус UPnP
- `regenerate_cookie_key()` — **НОВОЕ**: регенерировать ключ кук (аннулирует все сессии)

### Этап 8: Определение внешнего IP

**Файл**: `src-tauri/src/commands/webview.rs`

```rust
/// Получить внешний/публичный IP адрес с fallback
#[tauri::command]
pub async fn get_external_ip() -> Result<String, String> {
    let sources = vec![
        "https://api.ipify.org?format=text",
        "https://icanhazip.com",
        "https://ifconfig.me",
    ];

    for url in sources {
        match reqwest::get(url).await {
            Ok(resp) => {
                if let Ok(ip) = resp.text().await {
                    let ip = ip.trim().to_string();
                    if !ip.is_empty() {
                        return Ok(ip);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    Err("Не удалось получить внешний IP".to_string())
}
```

### Этап 9: Frontend UI

**Файл**: `src/components/WebViewPanel.vue`

```vue
<section class="settings-section">
  <h2>Безопасность</h2>

  <!-- Токен доступа -->
  <div class="setting-row">
    <label>Токен доступа:</label>
    <code class="token-code">{{ maskedToken }}</code>
    <button @click="copyToken" title="Копировать"><Copy /></button>
  </div>

  <div class="setting-row">
    <button v-if="!hasToken" @click="generateToken">
      Сгенерировать токен
    </button>
    <button v-else @click="regenerateToken">
      Перегенерировать токен
    </button>
  </div>

  <!-- Ключ сессии (НОВОЕ) -->
  <div class="setting-row">
    <label>Ключ сессии:</label>
    <button @click="regenerateCookieKey" class="danger-button">
      Регенерировать ключ сессии
    </button>
  </div>
  <span class="setting-hint">
    Регенерация ключа аннулирует все активные сессии внешних пользователей.
  </span>

  <!-- UPnP статус -->
  <div class="setting-row">
    <label class="checkbox-label">
      <input type="checkbox" v-model="settings.upnp_enabled"
             @change="saveUpnpEnabled()" />
      <span>Включить UPnP (автоматический проброс порта)</span>
    </label>
  </div>
  <span class="setting-hint">
    UPnP включается автоматически при первой генерации токена.
  </span>

  <!-- Внешний URL -->
  <div class="setting-row" v-if="hasToken">
    <label>Внешний URL:</label>
    <div class="url-display">
      <code class="url-code">{{ externalUrl }}</code>
      <button @click="showExternalUrl" title="Обновить внешний IP">
        <Globe />
      </button>
    </div>
  </div>

  <div class="setting-row">
    <label>Локальный URL (OBS):</label>
    <div class="url-display">
      <code class="url-code">{{ localUrl }}</code>
      <button @click="copyLocalUrl"><Copy /></button>
    </div>
  </div>
</section>
```

**Добавления в Script:**
```typescript
const externalIp = ref<string | null>(null)

const externalUrl = computed(() => {
  if (!externalIp.value || !settings.value.access_token) return ''
  return `http://${externalIp.value}:${settings.value.port}/?token=${settings.value.access_token}`
})

const localUrl = computed(() => {
  return `http://${localIp.value}:${settings.value.port}`
})

async function showExternalUrl() {
  try {
    externalIp.value = await invoke<string>('get_external_ip')
  } catch (e) {
    showError('Не удалось получить внешний IP: ' + (e as Error).message)
  }
}

async function regenerateCookieKey() {
  if (!confirm('Это выйдет из системы всех внешних пользователей. Продолжить?')) return
  try {
    await invoke('regenerate_cookie_key')
    showSuccess('Ключ сессии перегенерирован')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}
```

### Этап 10: Регистрация модулей

**Файл**: `src-tauri/src/webview/mod.rs`

```rust
mod server;
pub mod templates;
pub mod security;  // НОВЫЙ
pub mod upnp;      // НОВЫЙ
```

**Файл**: `src-tauri/src/lib.rs`

Зарегистрировать новые команды в `invoke_handler`.

---

## Критические файлы (Топ 5)

1. **`src-tauri/src/webview/server.rs`** — основной сервер, добавить auth/cookies/UPnP
2. **`src-tauri/src/webview/mod.rs`** — структура настроек
3. **`src-tauri/src/config/settings.rs`** — сохранение настроек
4. **`src-tauri/src/webview/security.rs`** (НОВЫЙ) — определение сети, валидация токена
5. **`src/components/WebViewPanel.vue`** — UI настроек безопасности

---

## Чеклист тестирования

- [ ] Локальная сеть (127.0.0.1, 192.168.x.x) работает без токена
- [ ] Внешний доступ без токена возвращает 401
- [ ] `/?token=xxx` устанавливает куку и разрешает доступ
- [ ] SSE подключение работает с auth кукой
- [ ] UPnP открывает порт на роутере (проверить в админке роутера)
- [ ] Генерация токена сохраняет в settings.json
- [ ] Перегенерация токена аннулирует старый
- [ ] Очистка UPnP при остановке сервера
- [ ] Регенерация cookie_key аннулирует сессии
- [ ] SSE reconnection работает после авторизации

---

## Заметки по безопасности

1. **Хранение токена**: Открытым текстом в settings.json (приемлемо для desktop app)
2. **Cookie key**: Генерируется UUID v4, хранится в settings.json
3. **Безопасность куки**: HttpOnly, SameSite=Lax (без Secure - только HTTP)
4. **Обход локальной сети**: Всегда разрешать приватные сети
5. **Constant-time сравнение**: `subtle` crate защищает от timing-атак

---

## Миграция настроек

Миграция автоматическая благодаря `#[serde(default)]`. У существующих пользователей:
- `access_token`: `null` (нужно сгенерировать)
- `upnp_enabled`: `false` (отключено по умолчанию)
- `cookie_key`: генерируется автоматически при первом запуске

---

## Flow внешнего доступа

1. **Первая настройка**: Пользователь нажимает "Сгенерировать токен"
   - Токен (UUID v4) генерируется и сохраняется в конфиг
   - Cookie key генерируется (если нет)
   - UPnP автоматически включается (пробрасывается порт на роутере)

2. **Получение внешнего URL**: Пользователь нажимает "Показать внешний URL"
   - Внешний IP получается из API
   - Отображается URL: `http://public-ip:10100/?token=uuid-здесь`
   - Также показывается локальный URL: `http://192.168.1.100:10100` (для OBS)

3. **Внешний доступ**: Пользователь открывает внешний URL
   - Сервер валидирует токен из query параметра
   - Устанавливается HttpOnly кука
   - SSE подключается с `withCredentials: true`

4. **Регенерация ключа**: Аннулирует все активные внешние сессии

---

## Обработка ошибок

- **UPnP не работает**: Лог warn, сервер продолжает (ручной проброс работает)
- **Неверный токен**: 401 ответ, закрытие SSE
- **Куки заблокированы**: Показать ошибку с инструкциями
- **Локальная сеть**: Всегда обходить проверку токена
- **Cookie key неверный**: Перегенерировать с подтверждением пользователя
