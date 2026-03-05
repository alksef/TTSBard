# WebView Source - Промежуточный итог

**Дата:** 2025-03-06
**Статус:** ✅ Реализация завершена (Этапы 1-4)

---

## Выполнено

### ✅ Этап 1: Базовая структура
- [x] Зависимости добавлены (axum, tokio-tungstenite, tower-http, futures)
- [x] Модуль webview создан (mod.rs, server.rs, websocket.rs, templates.rs)
- [x] WebViewSettings интегрирован в AppState

### ✅ Этап 2: События и интеграция с TTS
- [x] AppEvent::TextSentToTts добавлен
- [x] Отправка события из OpenAI, Silero, Local TTS
- [x] EventSender тип добавлен

### ✅ Этап 3: WebSocket сервер
- [x] WebSocket broadcasting реализован
- [x] HTML рендеринг с подстановкой CSS/JS/{{SPEED}}
- [x] Запуск сервера при старте приложения

### ✅ Этап 4: Настройки и UI
- [x] Tauri commands (get_webview_settings, save_webview_settings, get_local_ip)
- [x] Сохранение настроек в файлы (AppData/ttsbard/webview/)
- [x] WebViewPanel Vue компонент создан
- [x] Интеграция в Sidebar и App.vue

---

## Git Commits

1. `650f240` - docs: add WebView Source design document
2. `2bbd65c` - docs: add WebView Source implementation plan
3. `066b515` - docs: add webview-source idea summary
4. `90feb50` - feat: implement WebView Source feature (stages 1-4 complete)

---

## Следующие шаги (тестирование и доработки)

### 1. Сборка и тестирование
```bash
cd src-tauri && cargo build --release
```

### 2. Проверить функциональность:
- [ ] Сервер запускается на порту 10100
- [ ] WebSocket соединение устанавливается
- [ ] Текст отправляется при TTS
- [ ] HTML/CSS настройки сохраняются
- [ ] OBS Browser Source показывает текст

### 3. Возможные доработки:
- [ ] Добавить валидацию порта в UI
- [ ] Добавить предпросмотр HTML/CSS
- [ ] Добавить тестовую кнопку для проверки соединения
- [ ] Логирование WebSocket подключений
- [ ] Обработка ошибок при старте сервера

---

## Файлы созданы/изменены

### Новые файлы:
- `src-tauri/src/commands/webview.rs`
- `src-tauri/src/webview/mod.rs`
- `src-tauri/src/webview/server.rs`
- `src-tauri/src/webview/websocket.rs`
- `src-tauri/src/webview/templates.rs`
- `src/components/WebViewPanel.vue`

### Изменённые файлы:
- `src-tauri/Cargo.toml`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/events.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/tts/local.rs`
- `src-tauri/src/tts/openai.rs`
- `src-tauri/src/tts/silero.rs`
- `src/App.vue`
- `src/components/Sidebar.vue`

---

## Важно для продолжения

**Ключевая информация:**

1. **Порт по умолчанию:** 10100
2. **Bind address:** 0.0.0.0 (все интерфейсы)
3. **Хранилище:** AppData/ttsbard/webview/
4. **WebSocket endpoint:** ws://IP:PORT/ws
5. **HTTP endpoint:** http://IP:PORT/

**Интеграция с TTS:**
- Событие `AppEvent::TextSentToTts(String)`
- Отправляется из всех TTS провайдеров перед synthesizing
- Обрабатывается в lib.rs в background thread

**Архитектура:**
- Отдельный thread с собственным Tokio runtime
- Broadcast channel для WebSocket клиентов
- RwLock для настроек
- Файловое хранилище для HTML/CSS шаблонов
