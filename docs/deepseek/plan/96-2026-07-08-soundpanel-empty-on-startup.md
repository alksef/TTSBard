# План 96: Bug — звуковая панель пуста при старте, записи появляются только после добавления

- **Дата:** 2026-07-08
- **Тип:** bug (frontend, вероятно — порядок загрузки / race)
- **Симптом (от пользователя):** «при запуске в звуковой панели пусто. после добавления появляются
  старые записи»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`
- **Stage-файл:** пока не нужен — это точечный баг, не дизайн-решение.

---

## Что уже проверено Claude (НЕ нужно перепроверять — это факты)

1. **Файл на диске корректен.** `%APPDATA%\ttsbard\soundpanel_bindings.json` — новый формат
   `SoundSets`, `active_set_id` валиден, активный набор «Основной» содержит 3 binding (C, A, H).
   Пример реального содержимого:
   ```jsonc
   {
     "active_set_id": "6e62dbea-…",
     "sets": [
       { "id": "6e62dbea-…", "name": "Основной",
         "bindings": [ {"key":"C",…}, {"key":"A",…}, {"key":"H",…} ] }
     ]
   }
   ```
2. **Бэкенд грузит корректно.** `setup.rs:158` вызывает `load_bindings(&soundpanel_state)`
   → `storage.rs:43` парсит `SoundSets`, зовёт `state.replace_sets(sets)` (storage.rs:93),
   возвращает bindings активного набора. **Это происходит ДО `init_windows`** (setup.rs:183),
   т.е. state готов до показа окон.
3. **Команды возвращают активный набор:**
   - `sp_get_bindings` → `state.get_all_bindings()` → bindings активного набора (state.rs:183).
   - `sp_get_sets` → `state.get_sets()` (state.rs:198).
4. **State — единственный экземпляр,** `.manage()` один раз (lib.rs:254). Дублей нет.

⇒ Вывод: данные на диске и в бэкенд-state правильные. **Баг во фронте** — первичная загрузка в
`onMounted` получает пустоту/не то, а перезагрузка по событию `soundpanel-bindings-changed`
(которое триггерится `sp_add_binding` через `emit_soundpanel_bindings_changed`) подтягивает
корректные («старые») записи.

---

## Подозреваемые места (фронт)

### Кандидат A — `SoundPanelTab.vue` `onMounted` (основное окно, вкладка)
`SoundPanelTab.vue:287-289`:
```ts
onMounted(async () => {
  await loadSets()
  await loadBindings()
  ...
```
Выглядит правильно. **Но:** `loadBindings()` (133-142) зовёт `sp_get_bindings` — если он
возвращается ДО того, как бэкенд завершил `replace_sets` (race с асинхронным startup),
получаем пустоту. Маловероятно т.к. load_bindings синхронен и идёт до окон — но проверить.

### Кандидат B — `SoundPanelApp.vue` `onMounted` (плавающее окно) — **ВЕРОЯТНЕЕ**
`SoundPanelApp.vue:165-167`:
```ts
onMounted(async () => {
  await loadSets()
  await loadBindings()
```
Плавающее окно показывается по `Ctrl+Shift+F2` (`ShowSoundPanelWindow`). Если окно **создаётся
лениво при первом показе**, `onMounted` отрабатывает на свежем DOM — это должно работать.
**НО** если окно создаётся при старте (hidden) и `onMounted` срабатывает один раз тогда — к
моменту первого показа данные могут быть устаревшими/пустыми, а перерисовка идёт только по
событию. Проверить: **как создаётся окно** (`init_windows` / `show_soundpanel_window` в
`soundpanel_window.rs`) — `WebviewWindowBuilder` при старте или lazy.

### Кандидат C — `find_active_index()` bug (state.rs:62-73)
```rust
pub fn find_active_index(&self) -> usize {
    if !self.active_set_id.is_empty() {
        if let Some(idx) = self.sets.iter().position(|s| s.id == self.active_set_id) {
            return idx;
        }
    }
    if self.sets.is_empty() { 0 } else { 0 }   // ← обе ветки = 0
}
```
Используется в `add_binding`/`remove_binding` (state.rs:160, 173). Сам по себе не объясняет
«пусто при загрузке» (там используется `find_active()`, а не `find_active_index()`), но это
**латентный баг** — починить заодно (вернуть реальный индекс активного набора, не хардкод 0).

### Кандидат D — DTO проброс через unified settings
В ревью плана 94 зафиксировано: DTO `soundpanel_bindings` оставлен как `Vec<SoundBinding>`
(bindings активного набора) — проброс через `get_all_app_settings`. Если основной интерфейс
где-то ещё читает `appSettings.soundpanel_bindings` вместо `sp_get_bindings` — может быть
рассинхрон. Проверить `SoundPanelTab.vue` — в новом коде он зовёт `sp_get_bindings`
(`loadBindings`, line 136), а НЕ `appSettings.soundpanel_bindings`. Но `useAppSettings`
появляется (line 11) — убедиться что он не используется для bindings.

---

## Задача DeepSeek

### Этап 1 — диагностика (ОБЯЗАТЕЛЬНО сначала логи, не угадывать)
1. Открыть DevTools плавающего окна и основного. Воспроизвести: старт приложения →
   `Ctrl+Shift+F2` (плавающее окно) → проверить консоль `[SoundPanel] Loaded bindings: …`.
   Вкладка «Звуковая панель» основного окна → консоль `[SoundPanelTab] Loaded sets: …`.
2. Зафиксировать: что реально приходит в первом `onMounted` (пусто? не тот набор?) vs что
   приходит после `soundpanel-bindings-changed`.
3. Ответить: плавающее окно создаётся при старте или lazy? (`soundpanel_window.rs` →
   `show_soundpanel_window`, `init_windows`).

### Этап 2 — фиксы (по результатам диагностики)
- **C (обязательно):** починить `find_active_index()` — вернуть реальный индекс, не `0`.
  ```
  if let Some(idx) = sets.sets.iter().position(|s| s.id == self.active_set_id) { return idx; }
  sets.sets.is_empty().then(|| 0).unwrap_or(0)
  ```
  (Т.е. fallback на 0 только если нет совпадения — но вернуть position, а не 0.)
- **B (вероятный фикс):** если плавающее окно создаётся при старте и `onMounted` срабатывает
  один раз — перезагружать bindings/sets **при каждом показе** окна (слушать событие показа /
  `tauri://focus` / вызвать reload в `show_soundpanel_window` через `app_handle.emit` или
 emit при show). Либо: при показе окна форсить `emit_soundpanel_bindings_changed`, чтобы
  фронт перечитал.
- **A/D:** убрать любой путь чтения bindings из `appSettings.*` — единственный источник
  `sp_get_bindings`/`sp_get_sets`.

### Этап 3 — тесты
- Юнит-тест `find_active_index()`: активный набор не первый → возвращает его индекс (сейчас
  вернёт 0 — тест упадёт, после фикса пройдёт).
- Тест миграции НЕ трогать (уже зелёный).

---

## Верификация
- `cargo check` 0/0, `cargo test --lib` всё зелёное.
- `vue-tsc --noEmit` 0 ошибок.
- **Runtime (обязательно):** старт → плавающее окно показывает 3 binding (C, A, H) сразу,
  без добавления. Вкладка «Звуковая панель» — то же.

## Не делать
- Не переписывать формат `SoundSets` (он правильный).
- Не трогать миграцию (старый Vec → один Set — работает).
- Не «чинить» `load_bindings` на бэкенде без доказательства (логов) — там всё корректно.
