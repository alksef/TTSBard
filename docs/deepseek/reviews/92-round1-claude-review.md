# Review 92 round1 — план 92 (persist вкладок): APPROVED (после правок Claude)

**Дата:** 2026-07-08
**План:** `docs/deepseek/plan/92-2026-07-08-editor-tabs-persistence.md`
**Task:** `docs/deepseek/tasks/92-round1-01.md`
**Вердикт:** ✅ **APPROVED** — после 2 правок Claude (одна критичная bugfix, одна outside-scope revert).

## Что реализовал DeepSeek
- `src-tauri/src/tabs.rs` — `TabManager` (`EditorTab`/`TabsData`, `MAX_TABS=50`, `MAX_TAB_TEXT_LEN=100_000`,
  spawn_save, валидация, сброс невалидного `active_id`, `tabs_path()`). ✅ точно по паттерну HistoryManager.
- `src-tauri/src/commands/tabs.rs` — newtype `TabsState(Arc<TabManager>)` + `get_tabs`/`save_tabs`. ✅
- `lib.rs` / `commands/mod.rs` — регистрация модуля, `.manage(tabs_state)`, команды в handler. ✅
- `useEditorTabs.ts` — `init()` (hydrate с `isHydrated` guard), debounced auto-save (deep watch, 500мс),
  `flushSave()`. ✅
- `InputPanel.vue` — `initTabs()` на mount, `flushTabsSave()` на unmount + close-request handler. ⚠️ см. C1.
- `EditorTabs.vue` — title «Рабочие вкладки» (убрано «не сохраняются»). ✅

## Правки Claude (после ревью)

### CRITICAL C1 — close-handler ломал tray-поведение
DeepSeek реализовал `onCloseRequested` как `event.preventDefault()` + `currentWindow.destroy()`.
Но бэкенд `lib.rs:482-486` **уже** обрабатывает `CloseRequested` для main: `api.prevent_close()` +
`window.hide()` (сворачивание в трей). Фронтовой `destroy()` **уничтожал бы окно** вместо сворачивания
в трей → ломал tray-UX.

**Фикс:** убраны `preventDefault()`/`destroy()`. Handler теперь только `await flushTabsSave()` —
бэкендовский `prevent_close + hide` продолжает работать. Закомментировано почему (чтобы будущий
DeepSeek не «починил» обратно).

### OUTSIDE-SCOPE — SettingsAiPanel.vue
DeepSeek переделал spellcheck-секцию в трёхпозиционный переключатель (off/offline/online) — это
**вне плана 92** (persist вкладок). Сам DeepSeek отметил, что онлайн-вариант не имеет бэкенд-команды
(`check_spelling_online` отсутствует). По правилу «не позволять вне-scope правкам» — **reverted**
(`git checkout`). Эта правка может быть переиспользована позже в плане по spellcheck-source, но
отдельным task-файлом.

## Bug, найденный ТЕСТАМИ (не чекбоксами)
- `save_then_load_round_trip` падал при параллельном запуске: тесты делили один temp-path (per-PID).
  **Production-код корректен** (round-trip проходит в изоляции). Bug был в тестах (гонка shared path).
  **Фикс:** уникальный path per-test через `AtomicU64` counter.

## Добавлено Claude
- 5 юнит-тестов `tabs::tests::*` (history.rs тестов не имел — tabs стал первым с тестами):
  empty-load, round-trip, MAX_TABS truncate, MAX_TAB_TEXT_LEN truncate, invalid active_id reset.
  Все проходят (включая параллельный запуск).

## Сборка / верифика
- `cargo check` — 0 ошибок, 0 warnings.
- `cargo clippy --lib` (tabs.rs) — чисто.
- `npx vue-tsc --noEmit` — 0 ошибок.
- `cargo test --lib tabs::` — **5/5 passed**.
- End-to-end логика persist подтверждена тестами (round-trip через файл + in-memory).

## Осталось (runtime, не автоматизировано)
- GUI-проверка: создать 2 вкладки + текст → закрыть (в трей) → реальный выход → рестарт → восстановление.
  Юнит-тесты покрывают persist-логику; GUI-交互 требует ручного теста (создать/перезапустить).
- Зависит от того, что `onUnmounted` flush успевает до выхода через tray → `exit(0)`.
  `setup.rs:419` tray-меню exit зовёт `app.exit(0)` — `onUnmounted` может не успеть.
  Mitigation: debounce 500мс сохраняет большую часть; последние 500мс набора могут теряться при
  мгновенном exit. Приемлемо для «рабочих вкладок». Зафиксировать как известное ограничение.

## Итог
План 92 реализован корректно. Два бага пойманы ревью/тестами (не DeepSeek-чекбоксами): tray-поломка
и test-race. Сборка 0/0, тесты 5/5. Готово к коммиту (по запросу пользователя).
