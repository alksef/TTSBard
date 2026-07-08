# План 92: Persist вкладок редактора + восстановление после перезапуска

- **Дата:** 2026-07-08
- **Тип:** feature (backend + frontend)
- **Stage:** `docs/stage/12-editor-tabs-persistence.md` (читать обязательно — все решения там)
- **Подход:** отдельный `tabs.json` + новый `TabManager` (HistoryManager-стиль); фронт
  `useEditorTabs` → `invoke` + debounced auto-save + hydrate при старте + flush при выходе.
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`
- **Меняет решение stage 06** (in-memory → persistent). Без риска миграции настроек — отдельный файл.

---

## Цель

1. Вкладки редактора (`useEditorTabs.ts`) сейчас in-memory — теряются при перезапуске. Сохранять
   их в `%APPDATA%\ttsbard\tabs.json` и **восстанавливать** (вкладки + активная + порядок) при старте.
2. Бэкенд — отдельный файл (не settings.json), по образцу `HistoryManager` (`history.rs`).
3. Фронт — debounced auto-save (deep-watch) + синхронный flush при размонтировании/выходе.

## Структура данных

```jsonc
// %APPDATA%\ttsbard\tabs.json
{ "active_id": "uuid", "tabs": [ { "id":"uuid", "title":"Текст 1", "text":"..." }, ... ] }
```
- `active_id` — id активной вкладки; если не найден среди `tabs` → fallback на `tabs[0]`.
- Порядок в массиве = порядок вкладок.

## Бэкенд (Rust)

### Файл `src-tauri/src/tabs.rs` (новый) — по образцу `history.rs:54-116`
```rust
use parking_lot::RwLock;  // НЕ std, как history.rs
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditorTab {
    #[serde(default)] pub id: String,
    #[serde(default)] pub title: String,
    #[serde(default)] pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TabsData {
    #[serde(default)] pub active_id: String,
    #[serde(default)] pub tabs: Vec<EditorTab>,
}

pub struct TabManager {
    path: PathBuf,
    data: RwLock<TabsData>,
}
```
- `TabManager::new(path)` — `fs::read_to_string` → `serde_json::from_str` → `unwrap_or_default()`
  (как `history.rs:90-116`). Старых файлов нет → пустой `TabsData`.
- `load_all(&self) -> TabsData` — clone под `read` локом.
- `save_all(&self, data: TabsData)` — клонируем data, `std::thread::spawn` запись файла
  (как `spawn_save_phrases`, `history.rs:81-87`), не блокировать UI-поток.
- **Валидация** (урок SECURITY из review-018): в `save_all` (или в команде) отбрасывать вкладки
  сверх `MAX_TABS = 50`; `text` обрезать до `MAX_TAB_TEXT_LEN = 100_000` символов. Константы в
  начале файла. Названия полей для фронта (`id`/`title`/`text`/`active_id`) — snake_case-френдли,
  serde как есть.

### Команды `src-tauri/src/commands/tabs.rs` (новый)
```rust
#[tauri::command]
pub fn get_tabs(state: State<'_, AppState>) -> TabsData {
    state.tabs.load_all()
}

#[tauri::command]
pub fn save_tabs(state: State<'_, AppState>, data: TabsData) -> Result<(), String> {
    // валидация размера (MAX_TABS, MAX_TAB_TEXT_LEN) → state.tabs.save_all(data)
    Ok(())
}
```
> Решение: save-all (не гранулярные add/remove/rename) — фронт дебаунсит и шлёт весь массив.
> Проще и надёжнее; вкладок мало.

### Регистрация
- `AppState` (`state.rs`) — добавить поле `pub tabs: Arc<TabManager>` (или `pub tabs: TabManager`
  если TabManager владеет RwLock внутри; смотри как `HistoryManager` зарегистрирован — сделать так же).
- `setup.rs` (или где создаётся HistoryManager) — `TabManager::new(appdata.join("tabs.json"))`,
  `.manage()` + в `AppState`.
- `lib.rs` `invoke_handler` (строка ~238) — добавить `get_tabs`, `save_tabs`.
- `commands/mod.rs` — `pub mod tabs;`.

## Фронт (TypeScript/Vue)

### `src/composables/useEditorTabs.ts` — переключить на invoke
Текущая реализация (`useEditorTabs.ts:16-78`) — `ref<EditorTab[]>` в памяти. Изменения:
1. **Hydrate при создании:** сделать composable async-init ИЛИ добавить `async function init()`,
   вызываемый из `InputPanel.vue` `onMounted` (или `App.vue`):
   - `const data = await invoke<TabsData>('get_tabs')`.
   - Если `data.tabs.length === 0` → создать дефолтный `[{ id: genId(), title: 'Текст 1', text: '' }]`,
     `active_id` = его id.
   - Иначе `tabs.value = data.tabs`, `activeId.value = data.active_id || tabs.value[0].id`.
   - Валидация фронта: если `activeId` не среди tabs → первый.
2. **CRUD остаётся синхронным** (`create`/`close`/`select`/`rename`) — мгновенный отклик UI,
   без await. После каждого — триггер debounced-save (см. ниже).
3. **Auto-save:** `watch(tabs, () => scheduleSave(), { deep: true })` + `watch(activeId, ...)`.
   - `scheduleSave()` — debounce ~500мс, вызывает `invoke('save_tabs', { data: { active_id, tabs } })`.
   - Текст активной вкладки меняется при наборе в редакторе → deep-watch ловит → debounced save.
4. **Flush при выходе:** `onUnmounted` (в InputPanel или App, где живёт composable) — если есть
   pending save, выполнить **синхронно** (await без debounce) перед размонтированием. Также —
   зарегистрировать Tauri window `onCloseRequested` / listen `tauri://close-requested` в основном
   окне, чтобы flush до закрытия приложения (иначе последний unsaved текст теряется). Это
   **блокирующий момент** — заложить явно.

### `src/components/editor/EditorTabs.vue`
- Убрать подпись «(не сохраняются)» (`EditorTabs.vue:44` title). Заменить на «Рабочие вкладки»
  или аналогичное (без «не сохраняются»).

### Связка с InputPanel / TtsEditor — без изменений
`text = computed → active.value.text` (решение stage 06) остаётся. `TtsEditor.vue` не трогать.

## Поведение восстановления (главный сценарий — проверить в ревью)
1. Пользователь: 3 вкладки, набрал текст в каждой, активна 2-я → закрыл приложение.
2. (debounce сохранил + flush при выходе записал `tabs.json`).
3. Старт: бэкенд читает `tabs.json` → `TabManager`. Фронт `init()` → `get_tabs` → 3 вкладки,
   active_id=2-я. EditorTabs рендерит их; `TtsEditor` через `watch(modelValue)` подхватывает текст
   активной. Курсор — в начало (ожидаемо, см. stage 06 риск №1).

## Риски (для ревью — особо проверить)
1. **Гонка сохранения при выходе** — flush ДО закрытия. Если `onUnmounted` асинхронный и окно
   закрывается раньше → потеря. Решение: `onCloseRequested` prevent + await flush + allow.
2. **Debounce vs частый набор** — без debounce запись файла на каждый символ. Debounce 500мс обязателен.
3. **Конфликт окон** — табы только в основном окне (плавающее окно перехвата — отдельный ввод).
   Зафиксировать: composable живёт в основном окне.
4. **MAX_TABS / MAX_TAB_TEXT_LEN** — валидация на бэкенде (save_tabs); на фронте soft-warn.
5. **Идемпотентность save** — перезапись файла целиком; при падении mid-write возможна порча →
   писать во временный файл + rename (atomic), если тривиально; иначе — как history.rs (прямой write).

## Критерии готовности (Definition of Done)
- [ ] `tabs.rs` + `commands/tabs.rs` созданы, `get_tabs`/`save_tabs` зарегистрированы.
- [ ] `useEditorTabs.ts`: hydrate из `get_tabs` при старте + debounced auto-save + flush при выходе.
- [ ] EditorTabs.vue: подпись «не сохраняются» убрана.
- [ ] `cargo check` — 0 ошибок, 0 warnings.
- [ ] `npx vue-tsc --noEmit` — 0 ошибок.
- [ ] Сценарий восстановления (3 вкладки → рестарт → 3 вкладки + активная) работает end-to-end.
- [ ] Валидация размеров срабатывает (MAX_TABS=50, MAX_TAB_TEXT_LEN=100000).
- [ ] Нет regressions в существующем поведении табов (create/close/select/rename).

## Не делать (out of scope)
- Наборы/сеты вкладок (просто persist текущей модели).
- Синхронизация между окнами (табы только в основном окне).
- Миграция с localStorage (её не было).
