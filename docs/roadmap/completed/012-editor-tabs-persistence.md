# ROADMAP-012 — Сохранение вкладок редактора

**Дата:** 2026-07-08
**Статус:** `completed` — исследование и persistence вкладок завершены
**Решение:** persist через **отдельный `tabs.json`** (HistoryManager-стиль), новый
`TabManager` на бэкенде; фронт `useEditorTabs` переключается на `invoke`. Это **меняет**
ранее зафиксированное в `06` решение «только в памяти сессии».
**Связано:** `06-editor-tabs-multiple-texts.md` (исходная in-memory реализация), `05-phrase-history.md`
(HistoryManager — образец менеджера), планы `77`, `82`, `83` (текущая реализация табов).

## Цель (запрос пользователя)
- Сейчас вкладки редактора (`useEditorTabs.ts`) — **in-memory**: при перезапуске приложения
  все рабочие тексты теряются. `EditorTabs.vue:44` даже подписан `"Рабочие черновики (не сохраняются)"`.
- Нужно: **сохранять вкладки между запусками** и восстанавливать их (содержимое + активная
  вкладка + порядок) при старте.

> **Смена решения vs stage 06:** stage 06 сознательно выбрал «только в памяти сессии», чтобы не
> трогать бэкенд и избежать «граблей миграции как с `playback_pause`». Пользователь теперь явно
> требует persist. Поскольку хранилище — **отдельный файл `tabs.json`**, а НЕ поле в `settings.json`,
> риск миграции настроек отпадает (нет `#[serde(default)]`-граблей: tabs.json десериализуется с
> `unwrap_or_default()`, как phrase_history.json — `history.rs:103-106`).

---

## Решение (варианты рассмотрены — см. ответы пользователя)

**Хранить в `tabs.json`** (отдельный файл в `%APPDATA%\ttsbard\`, рядом с `phrase_history.json` /
`input_history.json`), новый бэкенд-менеджер `TabManager`.

Почему **не** в `settings.json`:
- Засоряет единый конфиг настройками-данными (вкладки — это пользовательский контент, не настройка).
- `settings.json` загружается весь целиком; массив из 10-20 текстов туда не лезет по смыслу.

Почему **не** `localStorage` (WebView2):
- Теряется при сбросе/обновлении WebView2, не бэкапится вместе с `%APPDATA%`, не переносимо.
- Архитектура проекта — все пользовательские данные в `%APPDATA%\ttsbard\` через Tauri.

Почему **не** SQLite — см. stage 13 (объём не оправдывает).

---

## Структура данных

```jsonc
// %APPDATA%\ttsbard\tabs.json
[
  { "id": "uuid", "title": "Текст 1", "text": "..." },
  { "id": "uuid", "title": "Приветствие", "text": "..." }
]
```
- Порядок элементов в массиве = порядок вкладок (как сейчас в `tabs.value: EditorTab[]`).
- Активная вкладка: хранить **отдельно** — либо поле `active_tab_id`, либо восстанавливать по
  позиции (см. «Открытые вопросы» ниже). Рекомендация: `active_tab_id` в том же файле обёрнут в
  объект, ИЛИ отдельный мини-файл. Простейший и совместимый вариант — **обёртка-объект**:
```jsonc
// tabs.json (итоговый вариант)
{ "active_id": "uuid", "tabs": [ { "id","title","text" }, ... ] }
```
  → `#[serde(default)]` на `active_id` (если id не найден среди tabs — берём первый).

> Обёртка-объект предпочтительнее голого массива: при добавлении метаданных (timestamp, pinned)
  не ломает формат; `unwrap_or_default()` даёт backwards-compat.

---

## Архитектура (бэкенд)

Новый менеджер по образцу `HistoryManager` (`history.rs:54-116`).

### Файл `src-tauri/src/tabs.rs` (новый)
```rust
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

impl TabManager {
    pub fn new(path: PathBuf) -> Self { /* read_or_default, как history.rs:90-116 */ }

    pub fn load_all(&self) -> TabsData { /* clone под локом */ }
    pub fn save_all(&self, data: TabsData) { /* spawn_save (thread), как spawn_save_phrases */ }
    pub fn get_active_id(&self) -> String { ... }
}
```
- `RwLock<parking_lot>` — как в `history.rs:59` (не `std`, единый стиль проекта).
- `spawn_save` в отдельном потоке — как `spawn_save_phrases` (`history.rs:81-87`), чтобы не
  блокировать UI-поток на запись файла.
- **Валидация** (урок SECURITY из review-018): ограничить `MAX_TABS` (напр. 50) и
  `MAX_TAB_TEXT_LEN` (напр. 100_000 символов) — отбрасывать/обрезать на границе команды.

### Команды `src-tauri/src/commands/tabs.rs` (новый)
Минимум — две команды (вся логика CRUD во фронте, бэкенд лишь persist):
```rust
#[tauri::command]
pub fn get_tabs(state: State<'_, AppState>) -> TabsData { state.tabs.load_all() }

#[tauri::command]
pub fn save_tabs(state: State<'_, AppState>, data: TabsData) -> Result<(), String> {
    // валидация размера → state.tabs.save_all(data)
}
```
> Решение: бэкенд **не хранит** гранулярное состояние (create/close/rename — фронтовые операции
> над массивом); фронт дебаунсит и вызывает `save_tabs(all)` при любом изменении. Это проще и
> надёжнее набора команд add/remove/rename, и соответствует «фронт — источник правды для UI,
> бэкенд — persist» (как settings-паттерн). Альтернатива (гранулярные команды) — см. риски.

### Регистрация
- `AppState` (`state.rs:68`) — добавить `pub tabs: Arc<TabManager>`.
- `setup.rs` / `lib.rs` — создать `TabManager::new(appdata.join("tabs.json"))`, `.manage()`.
- `invoke_handler` (`lib.rs:~238`) — `get_tabs`, `save_tabs`.

---

## Архитектура (фронт)

### `src/composables/useEditorTabs.ts` — переключить на invoke
Сейчас (`useEditorTabs.ts:16-78`) — `ref<EditorTab[]>` в памяти. Изменения:
1. **Инициализация** (было: один пустой таб): на старте `await invoke('get_tabs')` → если пусто,
   создать дефолтный `[{id, 'Текст 1', ''}]`; иначе hydrate `tabs` + `activeId`.
   - `create()`/`close()`/`select()`/`rename()` остаются синхронными (мгновенный отклик UI),
     но после каждого вызова (через `nextTick`/watch) триггерят **debounced** `save_tabs`.
2. **Авто-save**: `watch(tabs, () => debouncedSave(), { deep: true })` — debounce ~500мс, чтобы не
   писать файл на каждое нажатие клавиши в редакторе (текст таба меняется при наборе).
   - Писать весь `tabs` целиком (не дельты) — просто, файл маленький.
3. **`EditorTabs.vue`**: убрать подпись «(не сохраняются)».

### Связка с InputPanel (без изменений)
`text = computed → active.value.text` (`06` решение) остаётся. При наборе текст пишется в активный
таб → deep-watch срабатывает → debounced save. `TtsEditor.vue` не трогается.

---

## Восстановление после перезапуска (главный сценарий)
1. При `setup` бэкенд читает `tabs.json` → `TabManager` хранит в памяти.
2. Фронт `useEditorTabs` на `onMounted` (или при первом создании composable) → `get_tabs`.
3. Если `active_id` невалиден (нет среди tabs) — выбрать первый таб (fallback).
4. EditorTabs рендерит восстановленные вкладки; `TtsEditor` через `watch(modelValue)`
   подхватывает текст активной (как при обычном переключении таба — см. `06` риск №1).

---

## Риски / нюансы (для будущего плана)
1. **Производительность авто-save при наборе** — deep-watch на массив вкладок + debounce 500мс.
   Текст меняется на каждое нажатие; без debounce — запись файла на каждый символ. Debounce обязателен.
   При быстром наборе между нажатиями <500мс — сохранение откладывается до паузы. При закрытии окна —
   flush (принудительный save) в `onUnmounted`/before-quit, иначе последний unsaved текст теряется.
2. **Гонка сохранения при выходе** — если приложение закрывают во время debounce-окна, нужно
   синхронно сбросить save перед закрытием (Tauri `on_close` / flush в `onUnmounted`). Синхронная
   запись из UI-потока один раз при выходе — допустима (файл маленький). Это блокирующий момент
   реализации; заложить в план явно.
3. **Конфликт: основное окно + плавающее окно редактора** — если оба трогают табы одновременно,
   два `save_tabs` могут перетираться. Сейчас редактор один (основное окно); плавающее окно
   перехвата — отдельный поток ввода (не табы). Зафиксировать: табы живут только в основном окне.
   Если позже второе окно получит редактор — потребуется single-source-of-truth через события.
4. **Большие тексты в табах** — `MAX_TAB_TEXT_LEN` валидация на бэкенде (урок SECURITY review-018);
   на фронте — soft-warn если таб превышает лимит (как warning в TTS-провайдерах без ключа).
5. **Гранулярные команды vs save-all** — выбран save-all (проще). Если вкладок станет много (>50),
   сериализация всего массива на каждое изменение может стать тяжёлой. При текущих объёмах
   (≤20 вкладок, короткие тексты) — пренебрежимо. Зафиксировать порог пересмотра.
6. **Миграция существующих пользователей** — у них нет `tabs.json`; `read_or_default` → пустой
   TabsData → фронт создаёт дефолтный «Текст 1». Безболезненно.
7. **`active_id` хранение** — если хранить в `tabs.json` обёрткой, при ручной правке файла id может
   рассинхронизироваться; fallback на «первый таб» покрывает.

## Открытые вопросы (уточнить в плане)
- Сохранять ли `active_id` в `tabs.json` (обёртка-объект) ИЛИ восстанавливать всегда первый таб?
  Рекомендация: сохранять (UX — пользователь возвращается к той же вкладке). Если файл повреждён —
  первый.
- Лимиты: `MAX_TABS` (рек. 50), `MAX_TAB_TEXT_LEN` (рек. 100_000). Уточнить значения.

## Оценка трудозатрат
| Часть | Объём |
|---|---|
| `tabs.rs` (TabManager: read/save/spawn_save, валидация) | малый |
| `commands/tabs.rs` (get_tabs, save_tabs) + регистрация | малый |
| `useEditorTabs.ts` (hydrate on mount + debounced auto-save + flush on unmount) | средний |
| Удаление подписи «не сохраняются» | тривиально |
| Бэкенд-миграция старых конфигов | нет (read_or_default) |

Уровень ~ план 77. Реализация — через DeepSeek (CLAUDE.md workflow); Claude — план/ревью.

## KEY_DECISIONS
- **Хранилище:** отдельный `tabs.json` в `%APPDATA%\ttsbard\` (не settings.json, не localStorage).
- **Бэкенд:** новый `TabManager` по образцу `HistoryManager`; команды `get_tabs`/`save_tabs`
  (save-all, не гранулярные).
- **Фронт:** `useEditorTabs` hydrate при старте + debounced auto-save (deep-watch) + flush при выходе.
- **Меняет решение stage 06** (in-memory → persistent) — без риска миграции настроек, т.к. отдельный файл.
- **Валидация** размеров (MAX_TABS, MAX_TAB_TEXT_LEN) — урок SECURITY из review-018.
- **Активная вкладка** сохраняется (`active_id` в tabs.json) с fallback на первый таб.
