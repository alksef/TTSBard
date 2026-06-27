# Plan 72: Локальная история ввода + автодополнение слов

**Дата:** 2026-06-28
**Тип:** Feature plan (общий)
**Статус:** Draft (код пишет DeepSeek)
**Stage-исследование:** `docs/stage/02-local-history-autocomplete.md`
**Зависит от:** `71-codemirror-editor` (нужен каркас CM6 + `@codemirror/autocomplete`)
**Связано:** `73-hybrid-text-completion`

## Цель
Сохранять **persistent** слова, которые пользователь вводит повторно, и предлагать их в
автодополнении редактора: **по подстроке**, с приоритетом самых частых (взвешенный Trie).

## Решено пользователем
- Persistent: **да**, между запусками (расширяет `PROBLEMS.md:86`).
- Хранение: **Tauri-стор** (Rust), `%APPDATA%\ttsbard\input_history.json`.
- Структура поиска: **взвешенный Trie** (сортировка по `count`).
- Тип сопоставления: **по подстроке** (`indexOf`), как в IDE.

## Область изменений (общая)

### Backend (Rust)
1. **Модель данных:** serde-структура `InputHistory` (слова + частоты + lastUsed).
2. **Хранилище:** файл `%APPDATA%\ttsbard\input_history.json`; загрузка/сохранение по образцу
   `SettingsManager` / `save_replacements` (`commands/preprocessor.rs`).
3. **Trie-движок:** модуль для поиска по подстроке с взвешенной сортировкой (in-memory, под
   кэшем `RwLock`).
4. **Команды Tauri** (регистрация в `lib.rs` `invoke_handler`):
   - `get_history_suggestions(prefix: String, limit: usize) -> Vec<Suggestion>`
   - `add_history_entry(word: String)` / `record_history(text: String)` (токенизация, нормализация)
   - `clear_history()` (опц.)
5. **Ошибки:** `AppError` → `Result<T, String>` в командах.

### Frontend (Vue/TS)
1. **Composable:** `src/composables/useInputHistory.ts` — обёртка над invoke-командами
   (debounce на запрос подсказок).
2. **Autocomplete-источник для CM6:** расширение поверх `@codemirror/autocomplete`, берёт
   токен под курсором → запрашивает `get_history_suggestions` → рендерит попап.
3. **Запись истории:** при отправке текста (Enter/speak) вызывать `record_history`
   (токенизация на стороне Rust).

## Критерии приёмки
- Повторно введённые слова появляются в автодополнении после отправки.
- Подсказки ищутся по подстроке, сортируются по частоте.
- История переживает перезапуск приложения.
- Запрос подсказок дебаунсится (без лагов ввода).
- `vue-tsc` / `cargo check` проходят.

## Не входит
- Продолжение фраз (n-граммы/AI) → план 73.
- Hunspell-орфография — отдельная тема.

## Передано в DeepSeek
Детальный план → `docs/deepseek/plan/72-local-history-autocomplete.md`.
