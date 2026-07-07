# Ревью Claude — План 91, ЧАСТЬ A (после DeepSeek, итерация 91-A-01)

- **Дата:** 2026-07-08
- **Ревьюер:** Claude (не доверяет само-вердикту DeepSeek — см. `[[deepseek-review-checkboxes-unreliable]]`)
- **Task-итерация:** `docs/deepseek/tasks/91-A-01.md` (только ЧАСТЬ A: SoundPanel как окно)
- **Сборка (подтверждено Claude, не только DeepSeek):** `cargo check` → 0 errors / 0 warnings; `npx vue-tsc --noEmit` → exit 0.

## Verdict: APPROVED

Все 6 пунктов A1–A6 реализованы корректно. Сборка зелёная. Регрессий по логике
не обнаружено. Одно info-замечание (rustfmt-шум вне scope) — не блокирующее.

## Сверка по пунктам

### A1 — показ окна с фокусом ✅
`soundpanel_window.rs:show_soundpanel_window`: `window.show()?` → `window.set_focus()?`.
Clickthrough **снимается** на активной фазе (`set_ignore_cursor_events(false)`),
**восстанавливается** в `hide_soundpanel_window` (если `is_floating_clickthrough_enabled`).
Это даже лучше плана: восстановление при hide реализовано полностью.

### A2 — убрать interception_enabled из открытия панели ✅
`hotkeys.rs:handle_sound_panel`: `set_interception_enabled(true)` и связанные
debug-логи удалены. Осталось `set_active_window(SoundPanel)` + `emit(ShowSoundPanelWindow)`.

### A3 — DOM keydown в SoundPanelApp.vue ✅
`onKeydown(e)`:
- `Escape` → `closeWindow()`.
- Модификаторы (ctrl/shift/alt/meta) → игнор.
- `/^[A-Z]$/` → `find` биндинг: есть → `preventDefault` + `invoke('sp_play_binding',{key})` + `closeWindow`; нет → `showNoBinding(key)`.
- Прочие клавиши → игнор.
Listener добавлен в `onMounted`, убран в `onUnmounted`. Корректно.

### A4 — команда sp_play_binding + регистрация ✅
`bindings.rs:sp_play_binding`: валидация A-Z → `get_binding` → `play_sound` →
`emit(HideSoundPanelWindow)`. Зарегистрирована в `lib.rs:invoke_handler` и в `mod.rs` re-export.

### A5 — A-Z/Escape удалены из hook ✅
`hook.rs:soundpanel_keyboard_proc`: A-Z/Escape-логика удалена. Proc — pass-through
(`CallNextHookEx`). Инфраструктура hook (message pump, `SP_HOOK_STATE`,
`is_interception_enabled`) сохранена для ЧАСТИ B. `_vk_code` префикс — для будущего
(ЧАСТЬ B будет матчить NumPad/F-keys).

### A6 — blur → hide + восстановление clickthrough ✅
`lib.rs:on_window_event`: для `window.label()=="soundpanel"` при `Focused(false)` →
`hide_soundpanel_window(&app_handle, &app_state)`. Сигнатура совпадает
(`(&AppHandle, &AppState)`). `hide_soundpanel_window` восстанавливает clickthrough.

## Info-замечание (не блокирующее): rustfmt-шум вне scope

DeepSeek применил rustfmt к файлам, которые читал, но **не обязан был менять**:
`state.rs` (+99/−60 — почти весь дифф это перенос строк и реордеринг `use`),
`storage.rs`, `audio.rs`, `config/constants.rs` (удаление пустых строк), и
форматнинг logging-макросов в `soundpanel_window.rs`/`hotkeys.rs`.

**Безопасно:** логических изменений нет (сборка зелёная, поведение идентично).
**Но нежелательно:** раздувает дифф вне scope задачи, усложняет ревью и будущий
`git blame`. На следующую итерацию — в task-файле добавить явное «НЕ применять
rustfmt к файлам вне области задачи; правки только целевые».

> Примечание: `constants.rs` был `M` ещё до DeepSeek (в исходном `git status`
> сессии) — то изменение не от DeepSeek.

## Известные ограничения (из плана, не регрессии)
- `set_focus()` на always-on-top окне может сбросить эксклюзивный full-screen
  (known limitation, зафиксировано в плане 91, согласовано с пользователем).

## Итог

ЧАСТЬ A готова: SoundPanel работает как обычное окно (фокус → DOM keydown →
звук → auto-close; blur → hide; clickthrough снимается/восстанавливается).
Глобальный hook больше не блокирует A-Z. Можно коммитить ЧАСТЬ A и переходить
к ЧАСТИ B (Intercept — отдельный task-файл).

> Рекомендация: визуально проверить в реальном окне (Ctrl+Shift+F2):
> 1) A-Z → звук → закрылось; 2) Escape → закрылось; 3) клик мимо → закрылось;
> 4) clickthrough-режим — keydown всё равно работает (clickthrough снят).
> Это визуальные свойства, статический анализ их полностью не покрывает.
