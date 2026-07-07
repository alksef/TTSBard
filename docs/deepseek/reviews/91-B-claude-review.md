# Ревью Claude — План 91, ЧАСТЬ B (после DeepSeek, итерация 91-B-01)

- **Дата:** 2026-07-08
- **Ревьюер:** Claude (не доверяет само-вердикту DeepSeek — см. `[[deepseek-review-checkboxes-unreliable]]`)
- **Task-итерация:** `docs/deepseek/tasks/91-B-01.md` (только ЧАСТЬ B: Intercept)
- **Сборка (подтверждено Claude, не только DeepSeek):** `cargo check` → 0 errors / 0 warnings; `npx vue-tsc --noEmit` → exit 0.

## Verdict: APPROVED

Все 6 пунктов B1–B6 реализованы корректно. Сборка зелёная. Дифф строго в scope
(на этот раз **без** rustfmt-шума вне области — замечание из ЧАСТИ A учтено).
ЧАСТЬ A не регрессировала. Регрессий по логике не обнаружено.

## Сверка по пунктам

### B1 — persisted-конфиг intercept.json ✅
Новый `src-tauri/src/soundpanel/intercept.rs`: `InterceptSettings { enabled, bindings: Vec<InterceptBinding> }`,
`InterceptBinding { key, action }`, `load(path)`/`save(path)`. Модуль зарегистрирован в `mod.rs`.

### B2 — Intercept в SoundPanelState ✅
`state.rs`: поле `intercept: Arc<Mutex<InterceptSettings>>`, `new()` грузит `intercept.json`
(`load(&appdata_path)` → persist между запусками). Методы `get_intercept`,
`set_intercept_enabled` (persist + emit `InterceptionChanged`), `set_intercept_binding`,
`clear_intercept_binding` — все persist через `save()`. Drop mutex перед save — корректно
(без дедлока). Существующие поля/методы ЧАСТИ A не тронуты.

### B3 — run_action ✅
`hotkeys.rs:run_action(app_handle, action)`: match по 6 actions, для `show_soundpanel_window`
достаёт `AppState` через `try_state` (не параметром). `handle_*` стали `pub`. Hotkey-регистрации
продолжают работать напрямую.

### B4 — Intercept-логика в hook proc ✅ (центральный пункт)
`hook.rs:soundpanel_keyboard_proc`:
- `intercept.enabled` (persisted) проверяется; если выкл → `CallNextHookEx`.
- `vk_to_name(vk_code)` → find binding по `key` → `run_action(APP_HANDLE, &action)` →
  **`return LRESULT(1)`** (фильтруем, не отдаём в систему).
- Незабинженные клавиши (нет binding) → проваливается к `CallNextHookEx` (проходят насквозь).
- **Toggle не гаснет** после срабатывания (нет `set_intercept_enabled(false)`).
- `APP_HANDLE: OnceLock<AppHandle>` заведён, устанавливается в `initialize_soundpanel_hook`.
- Старый `is_interception_enabled()` (soundpanel runtime-флаг) больше не управляет hook —
  помечен `#[allow(dead_code)]` (оставлен, не warning).

### B5 — vk_to_name ✅
`intercept.rs:vk_to_name`: NumPad 0–9 (0x60..0x69), multiply/add/subtract/decimal/divide
(0x6A/0x6B/0x6D/0x6E/0x6F), F1–F24 (0x70..0x87). Прочее → `None`.

### B6 — команды + UI ✅
**Команды** (`bindings.rs`): `get_intercept_settings`, `set_intercept_enabled`,
`set_intercept_binding`, `clear_intercept_binding` — зарегистрированы в `lib.rs:invoke_handler`.
**UI** (`src/components/InterceptPanel.vue`): toggle + запись (NumPad/F-keys) + select из 6
actions + clear. Регистрация: `Panel` type расширен `'intercept'` (App.vue + Sidebar.vue),
кнопка «Перехват» с иконкой `Crosshair` ниже «Горячие клавиши», `<InterceptPanel v-show=...>`.

## Сверка с DoD плана 91 (ЧАСТЬ B, пп. 8–14)
8. `intercept.json` в `%APPDATA%/ttsbard/`, persist — ✅ (load в new, save в setters)
9. Toggle persist, восстанавливается при старте — ✅ (load в new)
10. Забинженная NumPad/F-клава не доходит до системы (LRESULT(1)) + action — ✅ (B4)
11. Незабинженные проходят насквозь — ✅ (CallNextHookEx fallthrough)
12. Actions = 6, переиспользуют handle_* — ✅ (run_action)
13. Работает, когда главное окно в фокусе — ✅ (WH_KEYBOARD_LL глобальный, не зависит от фокуса)
14. Сборка 0/0 — ✅

## Замечания (info, не блокирующие)
- (info) `is_interception_enabled()` теперь dead-code (помечен `#[allow(dead_code)]`). На
  будущее можно убрать совсем, если runtime-флаг soundpanel окончательно не нужен — но это
  не критично, явный allow чише, чем silent-удаление.
- (info) Визуальная проверка в реальном окне обязательна (см. ниже) — Intercept и
  блокировка клавиш это runtime/OS-поведение, статическим анализом полностью не покрывается.

## Итог

ЧАСТЬ B готова: Intercept-режим через обобщённый `WH_KEYBOARD_LL` hook, persisted
`intercept.json`, NumPad/F-keys → 6 actions (переиспользуют hotkey-хендлеры),
фильтрация забинженных клавиш от системы, панель «Перехват» в сайдбаре. Сборка зелёная.

Plan 91 (ЧАСТЬ A + B) полностью реализован. Можно коммитить ЧАСТЬ B.

> Рекомендация (визуальная, требует запуска приложения):
> 1. Панель «Перехват»: toggle сохраняется между запусками.
> 2. Забиндить NumPad1 → «show_main_window», включить Intercept → нажать NumPad1 вне
>    приложения → главное окно открылось, **символ не напечатался** в активном поле
>    (фильтрация работает).
> 3. Незабинженная NumPad-клавиша печатается нормально (проход насквозь).
> 4. Intercept работает, когда главное окно в фокусе.
