# DeepSeek Plan 72: Локальная история ввода + автодополнение слов

> **Для DeepSeek:** пиши реализацию сам. Здесь — инструкции (файлы/типы/сигнатуры/поведение),
> не готовый код. Общий план — `docs/plans/72-...`, контекст — `docs/stage/02-...`.
> Зависит от плана 71 (каркас CM6 + `@codemirror/autocomplete`).

## Контекст (Rust-стор)
- AppData: `%APPDATA%\ttsbard\` (рядом с `settings.json`).
- Образец сохранения файла: `src-tauri/src/commands/preprocessor.rs::save_replacements`
  (через `fs::write`, `fs::create_dir_all`, ошибка → `Result<T, String>`).
- Образец state/кэша: `AppState` (`src-tauri/src/state.rs`, `Arc<RwLock<...>>`),
  `SettingsManager` (`config/settings.rs`).
- Регистрация команд: `invoke_handler` в `src-tauri/src/lib.rs` (~строка 238).
- Ошибки: `AppError` (`src-tauri/src/error.rs`, thiserror), alias `Result<T>`.

## Что сделать

### Backend (Rust)
1. **Модель данных** — serde-структура для словаря истории: слово + счётчик частоты +
   `lastUsed` (timestamp). Продумай сериализацию в JSON.
2. **Хранилище** — файл `%APPDATA%\ttsbard\input_history.json`. Реализуй загрузку при старте
   и сохранение после изменений (по образцу `SettingsManager` / `save_replacements`).
   Создавай директорию при отсутствии.
3. **Trie-движок** — in-memory взвешенный Trie (по `count`), под кэшем `RwLock`. Реализуй:
   - добавление слова (incr count, обновить lastUsed);
   - поиск **по подстроке** (не только по префиксу) с сортировкой по частоте (desc);
   - возврат топ-N кандидатов.
4. **Tauri-команды** (зарегистрируй в `invoke_handler`, возвращают `Result<T, String>`):
   - `get_history_suggestions(prefix: String, limit: usize) -> Vec<Suggestion>`
     (`Suggestion`: слово + частота, для UI);
   - `record_history(text: String)` — токенизация текста (разделители, нижний регистр,
    фильтр шума/коротких токенов), обновление Trie и файла;
   - `clear_history()` (опционально).
5. **Запись истории** — вызывать `record_history` из фронтенда при отправке текста
   (Enter/speak). Токенизацию делай на стороне Rust.

### Frontend (Vue/TS)
1. **Composable `src/composables/useInputHistory.ts`** — обёртка над invoke-командами:
   `suggest(prefix, limit)` (с debounce, ~150–250мс), `record(text)`, `clear()`.
2. **Autocomplete-источник CM6** — расширение поверх `@codemirror/autocomplete`:
   - берёт текущий токен под курсором (текст до курсора до разделителя);
   - запрашивает `get_history_suggestions` через composable (debounced);
   - возвращает опции для попапа; навигация ↑/↓, принять Tab/Enter (не ломая Enter
     из quick-режима — Enter в попапе принимает подсказку, вне попапа — поведение плана 71).
3. **Подключение** — добавить autocomplete-расширение в расширения CM6 (план 71) внутри
   `TtsEditor.vue`.

## Ограничения / требования
- Запрос подсказок **дебаунсится** — без лагов и без спама invoke.
- Сопоставление — **по подстроке**, сортировка — **по частоте**.
- История **persistent** (переживает перезапуск).
- Без сети (локально).
- TypeScript строгий; Rust — следовать существующим паттернам (`AppError`, `State<'_, T>`).
- Не дублировать TTS-логику.

## Критерии готовности
- Повторно введённые слова появляются в автодополнении после отправки.
- Поиск по подстроке, сортировка по частоте.
- История переживает перезапуск приложения.
- `vue-tsc` / `cargo check` проходят.

---
**Статус: ВЫПОЛНЕНО** (28.06.2026)
- **Backend (Rust):**
  - `src-tauri/src/history.rs` — HistoryManager с Trie (in-memory), JSON persistence (`input_history.json`)
  - Поддержка record (слово + счётчик + lastUsed), suggest (по подстроке, сортировка по частоте), clear
  - `src-tauri/src/commands/history.rs` — Tauri-команды: `get_history_suggestions`, `record_history`, `clear_history`
  - HistoryState зарегистрирован в Tauri builder через `.manage()`
  - Команды добавлены в `invoke_handler`
- **Frontend (Vue/TS):**
  - `src/composables/useInputHistory.ts` — composable с suggest/record/clear + debounce
  - CM6 autocomplete-источник (в `TtsEditor.vue`) — запрос `get_history_suggestions`, показ результатов в попапе
  - `record_history` вызывается из `InputPanel.vue` после speak
- `vue-tsc`, `vite build`, `cargo check` проходят
