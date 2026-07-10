# Task 117-phase-B-03-editor: Выделение EditorService

План: `docs/plans/117-2026-07-11-appstate-decomposition-and-commands-refactoring.md` (читать обязательно).

## Описание задачи
Нам нужно вынести состояние редактора (история, спеллчек, препроцессор) из `AppState` в отдельную структуру `EditorService`.

## Шаги реализации

### 1. Создать `src-tauri/src/editor.rs`
Создай файл `src-tauri/src/editor.rs` и структуру `EditorService`:
```rust
use std::sync::Arc;
use parking_lot::Mutex;
use crate::preprocessor::TextPreprocessor;
use crate::history::HistoryManager;
use crate::spellcheck::SpellcheckManager;

pub struct EditorService {
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,
    pub history_manager: Arc<Mutex<Option<Arc<HistoryManager>>>>,
    pub spellcheck_manager: Arc<Mutex<Option<Arc<SpellcheckManager>>>>,
}

impl EditorService {
    pub fn new() -> Self {
        Self {
            preprocessor: Arc::new(Mutex::new(None)),
            history_manager: Arc::new(Mutex::new(None)),
            spellcheck_manager: Arc::new(Mutex::new(None)),
        }
    }
}
```
Зарегистрируй новый модуль в `src-tauri/src/lib.rs` (`mod editor;`).

### 2. `src-tauri/src/state.rs` — Обновление `AppState`
1. Замени поля в `AppState`:
   - Удали `pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>`
   - Удали `pub history_manager: Arc<Mutex<Option<Arc<crate::history::HistoryManager>>>>`
   - Удали `pub spellcheck_manager: Arc<Mutex<Option<Arc<crate::spellcheck::SpellcheckManager>>>>`
   - Добавь: `pub editor: Arc<crate::editor::EditorService>`
2. Обнови конструктор `AppState::new()`:
   ```rust
   let editor = Arc::new(crate::editor::EditorService::new());
   ```
   И вставь `editor` в структуру.

### 3. Обновление обращений (Каскадно)
Найди все места использования полей в бэкенде:
- `app_state.preprocessor` -> `app_state.editor.preprocessor`
- `app_state.history_manager` -> `app_state.editor.history_manager`
- `app_state.spellcheck_manager` -> `app_state.editor.spellcheck_manager`

Файлы для поиска и замены:
- `src-tauri/src/commands/mod.rs` (для TTS пайплайна)
- `src-tauri/src/commands/preprocessor.rs`
- `src-tauri/src/commands/history.rs`
- `src-tauri/src/commands/spellcheck.rs`
- `src-tauri/src/setup.rs`

## Верификация
1. `cargo check` — 0 ошибок (после выполнения Phase A).
2. В отчёте: покажи структуру `EditorService` и пример её вызова из `commands/preprocessor.rs`.
