# План 48: Поддержка MTProxy для Silero TTS

**Дата:** 2026-03-16
**Статус:** Черновик
**Связано:** Feature Request

## Обзор

Добавить поддержку MTProxy для Telegram Silero TTS, позволяя пользователям в регионах с ограничениями Telegram подключаться через MTProxy-серверы вместо SOCKS5 прокси.

## Предпосылки

MTProxy — это кастомный протокол прокси Telegram, предназначенный для:
- Обхода Deep Packet Inspection (DPI)
- Обхода цензуры в ограниченных регионах
- Более быстрого соединения по сравнению с SOCKS5

Реализация использует форк библиотеки `grammers` по адресу `git@github.com:alksef/grammers.git` с поддержкой MTProxy.

## Требования

### Требования к UI

1. **Выбор прокси для Silero TTS** (`TtsPanel.vue`)
   - Добавить опцию "MTProxy" в выпадающий список режима прокси
   - Опции: Нет | SOCKS5 | MTProxy

2. **Панель сети - секция MTProxy** (`NetworkPanel.vue`)
   - Создать отдельную панель для настройки MTProxy
   - Поля:
     - **Хост**: поле ввода (например, 127.0.0.1 или proxy.example.com)
     - **Порт**: поле ввода, по умолчанию 8888
     - **Ключ**: поле ввода в стиле пароля с кнопкой показать/скрыть
     - **DC ID**: опциональное поле внутри раскрываемой секции (по умолчанию: авто)
   - Кнопки: Тест | Сохранить

3. **Валидация**
   - Хост: обязательно, не пустой
   - Порт: 1-65535
   - Ключ: hex (32/34 символа) или base64 (24 символа)
   - DC ID: опционально, целое число если указано

### Требования к Backend

1. **Хранение настроек**
   - Добавить структуру `MtProxySettings` в `settings.rs`
   - Хранить в пути `tts.network.mtproxy`
   - Поля: host, port, secret, dc_id (опционально)

2. **Расширение режима прокси**
   - Добавить вариант `MtProxy` в enum `ProxyMode`
   - Обновить `ProxyMode::from_url()` для определения схемы mtproxy:// (опционально)

3. **Интеграция Grammers**
   - Обновить `Cargo.toml` для использования локального форка:
     ```toml
     grammers-mtsender = { git = "ssh://git@github.com/alksef/grammers.git", branch = "main", features = ["proxy", "mtproxy"] }
     ```
   - Или использовать path для локальной разработки:
     ```toml
     grammers-mtsender = { path = "../grammers/grammers-mtsender", features = ["proxy", "mtproxy"] }
     ```

4. **Команда тестирования MTProxy**
   - Новая команда: `test_mtproxy(host, port, secret, dc_id, timeout_secs)`
   - Процесс теста:
     - Валидация формата ключа
     - Создание тестового соединения с `MtProxyConfig`
     - Выполнение ping-запроса к Telegram
     - Измерение задержки
     - Возврат `TestResultDto`

5. **Интеграция с Telegram клиентом**
   - Изменить `init_with_proxy()` для обработки режима MtProxy
   - Создать `ConnectionParams` с полем `mtproxy` когда режим MtProxy
   - Передать `MtProxyConfig` в `SenderPool::with_configuration()`

6. **API команд**
   - `get_mtproxy_settings()` - Загрузить настройки MTProxy
   - `set_mtproxy_settings(host, port, secret, dc_id)` - Сохранить настройки MTProxy
   - `test_mtproxy(...)` - Проверить соединение MTProxy
   - Обновить `reconnect_telegram` для обработки режима MtProxy

## Структуры данных

### Настройки (settings.json)

```json
{
  "tts": {
    "network": {
      "proxy": {
        "proxy_url": "socks5://host:port"
      },
      "mtproxy": {
        "host": "proxy.example.com",
        "port": 8888,
        "secret": "dd0123456789abcdef0123456789abcdef",
        "dc_id": null
      }
    },
    "telegram": {
      "api_id": 12345,
      "proxy_mode": "mtproxy"
    }
  }
}
```

### MtProxySettings (backend)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MtProxySettings {
    pub host: Option<String>,
    pub port: u16,
    pub secret: Option<String>,
    pub dc_id: Option<i32>,
}

impl Default for MtProxySettings {
    fn default() -> Self {
        Self {
            host: None,
            port: 8888,
            secret: None,
            dc_id: None,
        }
    }
}
```

### ProxyMode (обновлённый)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    #[default]
    None,
    Socks5,
    MtProxy,  // Новый вариант
}
```

## Этапы реализации

### Фаза 1: Backend Core

1. **Обновление зависимостей** (`Cargo.toml`)
   - Изменить зависимости grammers для использования форка с feature mtproxy
   - Добавить `mtproxy` в список features

2. **Расширение настроек** (`src-tauri/src/config/settings.rs`)
   - Добавить структуру `MtProxySettings`
   - Добавить в `NetworkSettings`
   - Реализовать `set_mtproxy_settings()`, `get_mtproxy_settings()`
   - Обновить enum `ProxyMode` с вариантом `MtProxy`

3. **Команда теста MTProxy** (`src-tauri/src/commands/proxy.rs`)
   - Реализовать команду `test_mtproxy()`
   - Валидация формата ключа (hex/base64)
   - Создание тестового соединения через grammers `MtProxyConfig`
   - Выполнение ping-запроса
   - Возврат результата с задержкой

### Фаза 2: Интеграция с Telegram клиентом

4. **Обновление Telegram клиента** (`src-tauri/src/telegram/client.rs`)
   - Расширить `validate_proxy_url()` или создать `validate_mtproxy_config()`
   - Обновить `init_with_proxy()` для обработки `ProxyMode::MtProxy`
   - Создать `ConnectionParams` с полем `mtproxy`
   - Обновить `ProxyStatus` для отображения MTProxy

5. **Обновление команды reconnect** (`src-tauri/src/commands/telegram.rs`)
   - Обработка `ProxyMode::MtProxy` в логике переподключения
   - Загрузка настроек MTProxy при переподключении

### Фаза 3: API команд

6. **Команды MTProxy** (новые или расширение существующих)
   - `get_mtproxy_settings()` - Вернуть MtProxySettings
   - `set_mtproxy_settings(host, port, secret, dc_id)` - Сохранить настройки
   - `test_mtproxy(...)` - Проверить соединение
   - Экспортировать в `src-tauri/src/commands/mod.rs`

### Фаза 4: Frontend UI

7. **Секция MTProxy в Network Panel** (`src/components/NetworkPanel.vue`)
   - Добавить раскрываемую панель MTProxy
   - Поля ввода: Хост, Порт, Ключ (с показать/скрыть), DC ID (опционально)
   - Кнопки Тест и Сохранить
   - Загрузка настроек при монтировании
   - Валидация ввода

8. **Режим прокси в TtsPanel** (`src/components/TtsPanel.vue`)
   - Добавить "MTProxy" в селектор режима прокси для Telegram
   - Обновить логику переподключения для режима MtProxy

9. **Типы TypeScript** (`src/types/settings.ts`)
   - Добавить интерфейс `MtProxySettings`
   - Обновить тип `ProxyMode`
   - Расширить сигнатуры команд

### Фаза 5: Тестирование и документация

10. **Интеграционное тестирование**
    - Тест соединения MTProxy с реальным прокси
    - Проверка корректности переподключения
    - Тест автоопределения DC ID vs ручной

11. **Документация** (`docs/works/`)
    - Обновить proxy-settings.md с секцией MTProxy
    - Добавить устранение неполадок для частых проблем MTProxy

## Валидация формата ключа

Секретный ключ MTProxy может быть в нескольких форматах:

| Формат | Пример | Описание | Длина |
|--------|---------|-------------|--------|
| Hex (Simple) | `0123456789abcdef0123456789abcdef` | Без префикса, простой режим | 32 символа |
| Hex (DD-Secure) | `dd0123456789abcdef0123456789abcdef` | Префикс DD, обход DPI | 34 символа |
| Hex (EE-Prefix) | `ee0123456789abcdef0123456789abcdef` | Префикс EE | 34 символа |
| Base64 | `ASNFZ4mrze/+3LqYdlQyEA==` | В кодировке Base64 | 24 символа |

Логика валидации:
1. Проверить длину (24, 32 или 34 символа)
2. Если 34 символа, проверить префикс (dd/ee)
3. Сначала попытаться декодировать hex, потом base64
4. Проверить, что результат ровно 16 байт (или 17 с префиксом)

## Стратегия тестирования

### Unit-тесты
- Валидация формата ключа
- Сериализация/десериализация MtProxySettings
- Преобразование enum ProxyMode

### Интеграционные тесты
- Соединение MTProxy с тестовым сервером
- Ping-запрос через MTProxy
- Переподключение с режимом MTProxy

### UI-тесты
- Валидация формы
- Кнопка показать/скрыть ключ
- Функциональность кнопки тест

## Граничные случаи и considérations

1. **Выбор DC ID**
   - По умолчанию: None (авто из сессии)
   - Вручную: пользователь может указать при необходимости
   - Некоторые прокси поддерживают только определённые DC

2. **Конфликты режима прокси**
   - Только один тип прокси может быть активен одновременно
   - UI должен отключать другие конфигурации прокси при выборе MtProxy

3. **Экспозиция ключа**
   - Ключ должен быть замаскирован в логах
   - Показывать только последние 4 символа в статусе UI

4. **Миграция**
   - Миграция не нужна (новая функциональность)
   - Существующие настройки SOCKS5 не затрагиваются

5. **Feature flags Grammers**
   - Убедиться, что включены обе features: `proxy` и `mtproxy`
   - Форк должен быть совместим с текущей версией grammers

## Критерии успеха

- [ ] Настройки MTProxy можно сохранить и загрузить
- [ ] Тест соединения MTProxy работает корректно
- [ ] Telegram Silero TTS подключается через MTProxy
- [ ] UI позволяет переключаться между Нет/SOCKS5/MTProxy
- [ ] Опциональное поле DC ID работает корректно
- [ ] Валидация ключа предотвращает неверный ввод
- [ ] Документация обновлена

## Рекомендации

- Реализация MTProxy в grammers: `D:\RustProjects\grammers`
- Спецификация MTProxy: `grammers/docs/search/mtproxy/specification.md`
- Пример кода: `grammers/grammers-client/examples/mtproxy_test.rs`
- Текущая реализация прокси: `docs/plans/46-2026-0315-proxy-settings-implementation.md`

## Заметки

- Форк по адресу `git@github.com:alksef/grammers.git` должен быть доступен во время сборки
- Для локальной разработки использовать `path = "../grammers/grammers-mtsender"` в Cargo.toml
- В будущем рассмотреть возможность отправки изменений MTProxy в официальный репозиторий grammers
