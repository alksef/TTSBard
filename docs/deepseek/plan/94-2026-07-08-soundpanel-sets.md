# План 94: Саундпанель — наборы звуков (Sets) + переключение

- **Дата:** 2026-07-08
- **Тип:** feature (backend + frontend, два окна)
- **Stage:** `docs/stage/14-soundpanel-sets-and-inline-editing.md` (читать обязательно)
- **Подход:** Часть 1 (Sets) — модель наборов + оба окна показывают/переключают. **Без inline-редактирования**
  (то отдельный план 95/14-B). Унификация источника правды привязок.
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`
- **Блокирующий spike:** НЕТ (drag-drop подтверждён — Tauri v2 `onDragDropEvent.payload.paths`,
  нужен `dragDropEnabled:true` — но это для плана 95, не этого).

---

## Контекст (что есть сейчас)
- `SoundBinding` (`soundpanel/state.rs:18`): `{ key:char, description, filename, original_path? }`.
- `SoundPanelState` (`state.rs:31`): `bindings: Arc<Mutex<HashMap<char,SoundBinding>>>` — **один набор**.
  Геттер `get_all_bindings()` (`state.rs:123`) → отсортированный Vec.
- `soundpanel/storage.rs`: `load_bindings`/`save_bindings` → `soundpanel_bindings.json` (голый
  `Vec<SoundBinding>`). `copy_sound_file`/`delete_sound_file` управляют `%APPDATA%\ttsbard\soundpanel\`.
- `soundpanel/bindings.rs`: `sp_get_bindings`, `sp_add_binding`, `sp_remove_binding`, `sp_test_sound`,
  `sp_play_binding`, `sp_*_floating_*`.
- **⚠️ Двойной источник:** `SoundPanelTab.vue:204` читает `appSettings.soundpanel_bindings` (unified
  settings), а `SoundPanelApp.vue:47` (`sp_get_bindings`) — `SoundPanelState` (json). Синхрон через
  событие `soundpanel-bindings-changed`. Это долг — Sets должны жить в одном месте.

---

## Цель
1. Несколько **наборов (Sets)** звуков, каждый со своими привязками A-Z.
2. Переключение активного набора — в основном окне (SoundPanelTab) **и** в окне вызова
   (SoundPanelApp) — на лету.
3. Единый source-of-truth: Sets живут в `soundpanel_bindings.json` + `SoundPanelState`. Убрать дубль
   с `appSettings.soundpanel_bindings`.
4. Бэк-совместимость: старый `soundpanel_bindings.json` (голый Vec) → обернуть в один Set «Основной».

## Структура данных (бэкенд)

```jsonc
// %APPDATA%\ttsbard\soundpanel_bindings.json (новый формат)
{
  "active_set_id": "uuid",
  "sets": [
    { "id":"uuid", "name":"Основной", "bindings":[ {key,description,filename,original_path}, ... ] },
    { "id":"uuid", "name":"Мемы", "bindings":[ ... ] }
  ]
}
```

### `soundpanel/state.rs` — новые структуры
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoundSet {
    #[serde(default)] pub id: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub bindings: Vec<SoundBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoundSets {
    #[serde(default)] pub active_set_id: String,
    #[serde(default)] pub sets: Vec<SoundSet>,
}
```
- `SoundPanelState`: заменить `bindings: HashMap<char,SoundBinding>` на `sets: Arc<Mutex<SoundSets>>`.
- `get_all_bindings()` → bindings **активного** набора (чтобы `sp_play_binding`/плавающее окно не
  меняли интерфейс: возвращают Vec из active set). Если `active_set_id` невалиден → первый набор.
- Новый метод `get_sets()` → все наборы. `get_active_set()` → активный SoundSet. `set_active_set(id)`.

### `soundpanel/storage.rs` — миграция + новый формат
- `load_bindings`/`save_bindings` → работают с `SoundSets` (не голым Vec).
- **Миграция:** при чтении, если файл парсится как `Vec<SoundBinding>` (стый формат) → обернуть в
  один `SoundSet { id: gen(), name: "Основной", bindings }`, `active_set_id` = его id. Определить
  формат: попытка `serde_json::from_str::<SoundSets>`; если fail → `from_str::<Vec<SoundBinding>>`
  → обернуть. Бесконтактно для существующих пользователей.
- Сохранять только если >1 набора ИЛИ есть bindings (не плодить пустой файл у новых юзеров — как сейчас).

### `soundpanel/bindings.rs` — команды
- `sp_get_sets() -> SoundSets` (основное окно: список наборов).
- `sp_get_active_set() -> SoundSet` (окно вызова: что играть).
- `sp_set_active_set(id)` — переключить + emit `soundpanel-active-set-changed` (+ старое
  `soundpanel-bindings-changed` для совместимости).
- `sp_add_set(name) -> SoundSet`, `sp_rename_set(id,name)`, `sp_remove_set(id)`.
- Существующие `sp_add_binding`/`sp_remove_binding` — добавить параметр `set_id: Option<String>`
  (None → активный набор). emit `soundpanel-bindings-changed`.
- `sp_get_bindings` — оставить как alias активного набора (обратная совместимость с кодом, который
  его зовёт; переиспользует `get_all_bindings()`).
- Регистрация в `lib.rs`.

### Унификация источника (устранить дубль)
- Sets живут **только** в `soundpanel_bindings.json` + `SoundPanelState`.
- `SoundPanelTab.vue`: убрать чтение `appSettings.soundpanel_bindings` (`SoundPanelTab.vue:204`),
  перейти на `sp_get_sets`. Слушать `soundpanel-bindings-changed` / `soundpanel-active-set-changed`
  для обновления.
- `settings.rs` / unified config: `soundpanel_bindings` поле пометить deprecated (оставить чтение
  для миграции, не писать). Или убрать из DTO, если безопасно (проверить, кто ещё читает).

## Фронт

### Основное окно — `SoundPanelTab.vue`
- Над таблицей — **строка наборов** (как EditorTabs): `[Основной] [Мемы] +`, активный подсвечен.
  - Переключение → `sp_set_active_set`.
  - `+` → prompt имени → `sp_add_set` → выбрать новый активным.
  - На каждом наборе: rename (дабл-клик), remove (× с confirm).
- Таблица показывает bindings активного набора (как сейчас).
- Существующий диалог добавления (`sp_add_binding`) — с активным набором (set_id=None = active).

### Окно вызова — `src-soundpanel/SoundPanelApp.vue`
- В title-bar — **компактный переключатель набора**: стрелки `◀ Основной ▶` или dropdown.
  - Переключение → `sp_set_active_set` (без открытия основного окна).
  - Слушать `soundpanel-active-set-changed` → обновить selector + `loadBindings` активного набора.
- `bindings-grid` (`SoundPanelApp.vue:233`) — bindings активного набора (как сейчас).
- `onKeydown` (`:97`) → `sp_play_binding` — без изменений (играет из активного).

## Риски (для ревью — особо проверить)
1. **Миграция старого формата** — Vec → SoundSets. Тест: положить старый `soundpanel_bindings.json`
   (голый массив) → загрузка → один Set «Основной» с теми же bindings. Без паники.
2. **Двойной источник** — после унификации проверить, что `appSettings.soundpanel_bindings` больше
   нигде не читается как source-of-truth (только миграция). grep по коду.
3. **`sp_play_binding` после переключения** — переключили набор → нажали A → играет из нового
   активного. Проверить, что `get_binding(key)` берёт из активного набора.
4. **Удаление набора** — confirm; если удаляем активный → выбрать соседний/первый. Аудиофайлы НЕ
   удалять (могут использоваться в других наборах) — оставить (мусор, но безопасно). Зафиксировать.
5. **События** — `soundpanel-active-set-changed` (новое) + `soundpanel-bindings-changed` (существующее).
   Оба окна слушают. Не разрывать синхронизацию (правка в одном → отражается в другом).
6. **Имя набора дефолт/локализация** — «Основной» / «Набор N» (как EditorTabs «Текст N»).
7. **`active_set_id` persistence** — в `soundpanel_bindings.json`; при повреждении → первый набор.
8. **Окно вызова + clickthrough** — НЕ относится к этому плану (clickthrough-конфликт с редактированием
   — план 95). Здесь только просмотр/переключение; clickthrough работает как раньше.

## Критерии готовности (Definition of Done)
- [ ] `SoundSet`/`SoundSets` структуры + `SoundPanelState` держит наборы (не один HashMap).
- [ ] `storage.rs`: миграция старого Vec → SoundSets; save/load нового формата.
- [ ] Команды `sp_get_sets`, `sp_get_active_set`, `sp_set_active_set`, `sp_add_set`,
  `sp_rename_set`, `sp_remove_set` + регистрация.
- [ ] `sp_add_binding`/`sp_remove_binding` с `set_id`.
- [ ] Унификация: `SoundPanelTab` читает через `sp_*`, дубль с appSettings убран.
- [ ] Основное окно: строка наборов + CRUD + переключение.
- [ ] Окно вызова: переключатель набора + обновление grid.
- [ ] `cargo check` 0/0; `vue-tsc --noEmit` 0.
- [ ] Миграция: старый json → один Set без паники.
- [ ] End-to-end: 2 набора с разными A-привязками → переключение в окне вызова → A играет разный звук.
- [ ] Нет regressions (добавление/удаление звука, appearance, clickthrough, intercept).

## Не делать (out of scope — план 95 / 14-B)
- Inline-редактирование в окне вызова (drag-drop, режим редактирования).
- Авто-выключение clickthrough в режиме редактирования.
- Удаление аудиофайлов при удалении набора (оставить как есть).
