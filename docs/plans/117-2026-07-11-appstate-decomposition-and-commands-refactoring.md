# Plan 117: Декомпозиция AppState и рефакторинг команд бэкенда

**Дата:** 2026-07-11  
**Статус:** Запланировано к поэтапной реализации  
**Сложность:** Высокая (структурный рефакторинг бэкенда)

---

## 1. Цели
1. Разбить гигантский файл `src-tauri/src/commands/mod.rs` (1385 строк) на доменные файлы команд.
2. Выделить доменные сервисы из `AppState` для снижения связанности (loose coupling) и инкапсуляции блокировок.
3. Организовать TTS-pipeline (`speak_text_internal`) в последовательную цепочку обработчиков.
4. Допустить параллельную разработку подсистем бэкенда.

---

## 2. Поэтапный план реализации

### Фаза А: Разделение `commands/mod.rs` на подмодули (Последовательно)
На этой фазе мы разделяем логику одного файла на подмодули. Параллельный запуск на этой фазе невозможен, так как все изменения затрагивают один файл `commands/mod.rs`.

1. **Создание подмодулей команд:**
   - `src-tauri/src/commands/audio.rs` (аудио-устройства, громкость, переключение выходов)
   - `src-tauri/src/commands/ai.rs` (настройки AI, запросы коррекции)
   - `src-tauri/src/commands/webview.rs` (запуск/остановка WebView сервера, шаблоны, UPnP)
   - `src-tauri/src/commands/twitch.rs` (подключение, отправка сообщений, статус)
   - `src-tauri/src/commands/core.rs` (квит, TTS pipeline `speak_text_internal`)
2. **Очистка `commands/mod.rs`:**
   В основном файле остаются только декларации модулей (`pub mod`) и ре-экспорт (`pub use`).

---

### Фаза B: Выделение доменных сервисов (Параллельно)
После того как команды разнесены по разным файлам, мы можем запустить **несколько агентов параллельно**, так как они будут работать с полностью изолированными наборами файлов.

* **Поток 1 (Twitch Service):**
  - Файлы: `src-tauri/src/twitch/service.rs`, `src-tauri/src/commands/twitch.rs`.
  - Задача: Обернуть `twitch_settings`, `twitch_connection_status`, `twitch_event_tx` в `TwitchService`.
* **Поток 2 (WebView Service):**
  - Файлы: `src-tauri/src/webview/service.rs`, `src-tauri/src/commands/webview.rs`.
  - Задача: Обернуть `webview_settings`, `webview_event_sender` в `WebViewService`.
* **Поток 3 (Editor/Storage Service):**
  - Файлы: `src-tauri/src/editor/service.rs`, `src-tauri/src/commands/tabs.rs`, `src-tauri/src/commands/history.rs`.
  - Задача: Обернуть `history_manager`, `spellcheck_manager`, `preprocessor` в `EditorService`.

---

### Фаза C: Рефакторинг TTS Pipeline (Последовательно)
Разбиение `speak_text_internal` (в `commands/core.rs`) на этапы.
1. Создание конвейера: `text` → `Preprocess` → `AiCorrect` → `TtsSynthesize` → `AudioPlayback` → `SaveHistory`.
2. Повышение отказоустойчивости: если шаг `AiCorrect` завершился с ошибкой, конвейер падает до синтеза с исходным текстом (fallback), а не крашит всю задачу.

---

## 3. Критерии готовности
- Все файлы успешно компилируются на целевых платформах.
- Тесты бэкенда (`cargo test`) проходят успешно.
- В `AppState` отсутствуют прямые публичные блокировки (`Arc<Mutex<...>>` / `Arc<RwLock<...>>`) для внутренних компонентов.
- Фронтенд продолжает работать без изменений сигнатур IPC команд.
