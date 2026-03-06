# Code Review Fixes #001 - 2026-03-07

## Обзор
План исправления замечаний из код-ревью #001. Всего 21 замечание: 5 критичных, 7 некритичных, 4 оптимизации, 5 ИБ.

## Приоритет 1: Критичные проблемы безопасности (CRITICAL + SECURITY)

### 1.1. Thread-safe Windows Hook State
**Проблема**: `static mut` переменные без синхронизации в `src-tauri/src/hook.rs:24-37`
**Файлы**: `src-tauri/src/hook.rs`
**Задачи**:
- [ ] Заменить `static mut` на `lazy_static` или `once_cell::sync::Lazy`
- [ ] Использовать `AtomicBool` для флагов состояния
- [ ] Обернуть сложные состояния в `Arc<Mutex<>>` с `try_lock()` в callback
- [ ] Добавить логирование при ошибках блокировки

### 1.2. Deadlock Prevention
**Проблема**: Несогласованный порядок блокировки мьютексов в `src-tauri/src/state.rs`
**Файлы**: `src-tauri/src/state.rs`
**Задачи**:
- [ ] Определить и задокументировать иерархию блокировок
- [ ] Заменить блокировки на `try_lock()` с retry логикой
- [ ] Рассмотреть переход на `parking_lot::Mutex` (реentrant safe)
- [ ] Добавить timeout для всех блокировок

### 1.3. Thread Pool Management
**Проблема**: 5 неограниченных `thread::spawn` в `src-tauri/src/lib.rs`
**Файлы**: `src-tauri/src/lib.rs`
**Задачи**:
- [ ] Создать bounded thread pool с `available_parallelism()`
- [ ] Реализовать graceful shutdown с thread joining
- [ ] Добавить обработку ошибок при создании потоков
- [ ] Рассмотреть `tokio::task::spawn` для async контекста

### 1.4. IRC Injection Prevention
**Проблема**: Некорректная санитизация IRC сообщений в `src-tauri/src/twitch/client.rs`
**Файлы**: `src-tauri/src/twitch/client.rs`
**Задачи**:
- [ ] Удалить все `\r\n` из входящих сообщений
- [ ] Экранировать `\0` и другие управляющие символы
- [ ] Ограничить длину сообщения 500 символами
- [ ] Добавить юнит-тесты для санитизации

### 1.5. FFI Error Handling
**Проблема**: Windows API вызовы без проверки HRESULT
**Файлы**: `src-tauri/src/hook.rs`
**Задачи**:
- [ ] Проверять все возвращаемые значения Windows API
- [ ] Логировать ошибки с кодами
- [ ] Возвращать `Result` вместо silent failures
- [ ] Добавить тесты для ошибочных сценариев

## Приоритет 2: Остальные SECURITY

### 2.1. TTS Rate Limiting
**Проблема**: Нет ограничения запросов к TTS API
**Файлы**: `src-tauri/src/commands/mod.rs`, `Cargo.toml`
**Задачи**:
- [ ] Добавить крейт `governor` для rate limiting
- [ ] Реализовать token bucket: 10 запросов/минуту, 100/час
- [ ] Добавить очередь с приоритетами
- [ ] Экспоненциальный backoff при ошибках

### 2.2. TLS Certificate Validation
**Проблема**: Обход валидации сертификатов в `src-tauri/src/twitch/client.rs`
**Файлы**: `src-tauri/src/twitch/client.rs`
**Задачи**:
- [ ] Явно включить верификацию сертификатов
- [ ] Рассмотреть certificate pinning для twitch.tv
- [ ] Добавить конфигурацию для development/production

### 2.3. Timing Attack Prevention
**Проблема**: Уязвимость в `src-tauri/src/telegram/client.rs`
**Файлы**: `src-tauri/src/telegram/client.rs`
**Задачи**:
- [ ] Использовать constant-time сравнение для кодов
- [ ] Добавить случайный jitter к timeout
- [ ] Унифицировать время ответа для success/failure

## Приоритет 3: Оптимизации

### 3.1. Аудио данные
**Проблема**: Избыточное клонирование в `src-tauri/src/audio/player.rs`
**Файлы**: `src-tauri/src/audio/player.rs`
**Задачи**:
- [ ] Использовать `Arc<Vec<u8>>` для аудио данных
- [ ] Устранить `.clone()` при передаче устройствам

### 3.2. Hook Allocator Pressure
**Проблема**: Heap аллокация в callback в `src-tauri/src/hook.rs`
**Файлы**: `src-tauri/src/hook.rs`
**Задачи**:
- [ ] Использовать thread-local storage для keyboard_state
- [ ] Переиспользовать буфер между вызовами

### 3.3. Mutex Consolidation
**Проблема**: Двойная блокировка в `src-tauri/src/state.rs`
**Файлы**: `src-tauri/src/state.rs`
**Задачи**:
- [ ] Объединить связанные поля в одну структуру
- [ ] Использовать один `RwLock<T>` вместо нескольких мьютексов
- [ ] Стандартизировать на `tokio::sync::RwLock` или `parking_lot::RwLock`

### 3.4. JSON Caching
**Проблема**: Сериализация в tight loop в `src-tauri/src/webview/websocket.rs`
**Файлы**: `src-tauri/src/webview/websocket.rs`
**Задачи**:
- [ ] Закэшировать формат строки или переиспользовать сериализатор

## Приоритет 4: Некритичные улучшения

### 4.1. Error Handling Standardization
**Проблема**: Смешивание `unwrap()` и `if let Ok()`
**Файлы**: `src-tauri/src/settings.rs`, `src-tauri/src/state.rs`
**Задачи**:
- [ ] Заменить `.unwrap()` на `?` или `map_err()`
- [ ] Стандартизировать обработку ошибок
- [ ] Добавить логирование вместо panic

### 4.2. Clone Reduction
**Проблема**: Избыточное клонирование строк в `src-tauri/src/state.rs`
**Файлы**: `src-tauri/src/state.rs`
**Задачи**:
- [ ] Возвращать `String` напрямую из guard
- [ ] Использовать `Cow<'_, str>` где уместно

### 4.3. Must Use Attributes
**Проблема**: Отсутствие `#[must_use]` на fallible методах
**Файлы**: `src-tauri/src/tts/openai.rs` и другие TTS провайдеры
**Задачи**:
- [ ] Добавить `#[must_use]` всем методам, возвращающим `Result`

### 4.4. Vue Type Safety
**Проблема**: Type assertion в `src/components/TwitchPanel.vue`
**Файлы**: `src/components/TwitchPanel.vue`
**Задачи**:
- [ ] Использовать discriminated unions для статусов
- [ ] Добавить runtime валидацию с zod
- [ ] Исправить cleanup для event listeners

## Зависимости

- Задачи 1.1 и 1.2 связаны (hook state требует deadlock-safe мьютексов)
- Задачи 1.3 и 1.4 независимы, могут выполняться параллельно
- Задачи 3.1 и 3.3 связаны с 1.2 (консолидация мьютексов)

## Критерии завершения

- [ ] Все CRITICAL проблемы исправлены
- [ ] Все SECURITY уязвимости закрыты
- [ ] Нет `static mut` в коде
- [ ] Нет `.unwrap()` на hot path
- [ ] Все мьютексы имеют deadlock prevention
- [ ] Rate limiting реализован и протестирован
- [ ] Юнит-тесты добавлены для критичных изменений
