# План 91: Переработка клавиатурного ввода — SoundPanel как окно + Intercept как доп. фича

**Дата:** 2026-07-08
**Stage:** `docs/stage/11-keyboard-input-mechanism-redesign.md`
**Заметка:** `docs/notes/keyboard-input-two-stage-mechanism.md`
**Реализует:** через DeepSeek (`opencode run`); задача/итерации — `docs/deepseek/tasks/91-*`, ревью — `docs/deepseek/reviews/91-*`.

> ⚠️ **НЕ доверять само-вердикту DeepSeek `[x]`.** Проверять по реальному диффу и сборке (`vue-tsc --noEmit`, `cargo check`).

---

## 0. Решения (согласовано с пользователем)

| Развилка | Решение |
|----------|---------|
| **Stage 1: как ловить клавиши в SoundPanel** | **1A — фокус + DOM keydown + auto-hide по blur.** `show()` + `set_focus()`; окно ловит A-Z через `keydown`; после звука/escape → auto-close; blur → auto-hide. |
| **Clickthrough в активной фазе** | Снимать на время фокуса (panel не в clickthrough, пока активна); восстанавливать после закрытия. |
| **Stage 2: механизм Intercept** | **Обобщить существующий `WH_KEYBOARD_LL` hook.** Не `tauri-plugin-global-shortcut`. |
| **Stage 2: хранилище** | **Отдельный `intercept.json`** (по аналогии с `soundpanel_bindings.json`). |
| **Stage 2: диапазон клавиш** | **NumPad + F-keys** (VK_NUMPAD0..9, VK_MULTIPLY/ADD/SUBTRACT/DECIMAL/DIVIDE, VK_F1..VK_F24). |
| **Stage 2: actions** | Переиспользовать существующие хоткей-хендлеры (вызов окон + команды playback). Звук/TTS-фразы как actions — **позже**, не в этом плане. |
| **Known limitation** | `set_focus()` на always-on-top окне может сбросить эксклюзивный full-screen. Зафиксировано как known limitation; borderless-windowed стерпит. |

---

## 1. Контекст (опорные точки кода)

### Stage 1 (SoundPanel)
- Окно: `tauri.conf.json` label `soundpanel` (450×225, transparent, alwaysOnTop, **видим без фокуса**).
- Vue: `src-soundpanel/SoundPanelApp.vue` — уже грузит биндинги (`sp_get_bindings`), есть `closeWindow()`, `showNoBinding()`, `clickthroughEnabled`. **Нет `keydown`-обработчика.**
- Show: `src-tauri/src/soundpanel_window.rs:show_soundpanel_window()` — `window.show()` **без `set_focus()`**.
- Открытие сегодня: `hotkeys.rs::handle_sound_panel()` → `set_interception_enabled(true)` + `emit(ShowSoundPanelWindow)`.
- Hook (убираемый из основного пути): `src-tauri/src/soundpanel/hook.rs:soundpanel_keyboard_proc()` — ловит A-Z глобально, после звука гасит перехват и шлёт `HideSoundPanelWindow`.

### Stage 2 (Intercept)
- Hook: тот же `hook.rs` + `state.rs` (`SoundPanelState`: `interception_enabled`, `bindings`, `emit_event`).
- Actions (переиспользовать): `hotkeys.rs` — `handle_main_window`, `handle_sound_panel`, `handle_playback_control_window`, `handle_playback_pause/stop/repeat`.
- AppEvent: `events.rs` — `InterceptionChanged(bool)` **уже есть**; `Show*Window` есть.
- Settings DTO: `config/dto.rs:524 GeneralSettingsDto.interception_enabled` — **runtime-флаг** (не persisted). Для Intercept нужно **новое persisted-состояние**.
- Sidebar-паттерн: `src/components/Sidebar.vue` (`Panel` type, `sidebarGroups`), `src/components/HotkeysPanel.vue` (invoke, recording, error display). Запись клавиши: см. `HotkeysPanel.vue:handleKeyDown/handleKeyUp`.

---

## 2. Объём (2 части; можно реализовать последовательно)

### ЧАСТЬ A — SoundPanel как окно (Stage 1)

**A1. Rust: показывать окно с фокусом, не включая перехват.**
- `soundpanel_window.rs:show_soundpanel_window()` — после `window.show()` добавить `window.set_focus()?`.
- `hotkeys.rs:handle_sound_panel()` — **убрать** `sp_state.set_interception_enabled(true)`. Перехват для панели больше не нужен (клавиши ловит само окно). Оставить `emit(ShowSoundPanelWindow)` и `set_active_window(SoundPanel)`.
- Проверить: `ActiveWindow::SoundPanel` ещё где-то используется для логики перехвата? Если только в hook — не трогать (hook остаётся для Intercept, см. ЧАСТЬ B, но A-Z-ветка убирается).

**A2. Vue: ловить A-Z в окне, играть звук, закрываться.**
В `SoundPanelApp.vue`:
- `onMounted`: навесить `window.addEventListener('keydown', onKeydown)`. Запомнить `unlisten` для `onUnmounted`.
- `onKeydown(e)`:
  - `Escape` → `closeWindow()`. return.
  - Игнорировать, если есть модификаторы (ctrl/shift/alt/meta) — это не «звуковая» клавиша.
  - `key = e.key.toUpperCase()`. Если `A..Z`:
    - Найти биндинг в `bindings.value` по `key`.
    - Есть → `invoke('sp_play_binding', { key })` (см. A3) → `closeWindow()`.
    - Нет → `showNoBinding(key)` (без закрытия, как сегодня; остаётся в фокусе).
  - Прочие клавиши — игнорировать.
- ** НЕ** вызывать `set_ignore_cursor_events` отсюда.

**A3. Rust: команда «проиграть биндинг по клавише» + авто-закрытие.**
- Новая Tauri-команда `sp_play_binding(key: String)` в `src-tauri/src/soundpanel/bindings.rs` (или commands): берёт `SoundPanelState`, `get_binding(char)`, если есть — `play_sound` + `emit(HideSoundPanelWindow)` (переиспользует существующий hide-путь). Если нет — `Err` (Vue уже проверил локально, но защита).
- Команду зарегистрировать в `lib.rs`/`commands/mod.rs` (`invoke_handler`).
- `HideSoundPanelWindow` уже скрывает окно (см. `setup.rs` event-loop + `hide_soundpanel_window`).

**A4. Auto-hide по blur.**
В `SoundPanelApp.vue` (или в `soundpanel_window.rs` на Rust-стороне — предпочтительнее Rust, надёжнее):
- Rust: в `setup.rs`/`soundpanel_window.rs` слушать `on_window_event` для `soundpanel` → `WindowEvent::Focused(false)` → вызвать `hide_soundpanel_window()`. **НЕ** закрывать, если blur вызвался из-за всплывашки transparency-control (внутри окна). Уточнить: blur-фокус уходит только за пределы webview — внутренние элементы не триггерят `Focused(false)`.
- **Снимать clickthrough на время фокуса:** если `floating_clickthrough` включён, при `show_soundpanel_window` вызвать `window.set_ignore_cursor_events(false)` на время показа; при `hide_soundpanel_window` — восстановить `set_ignore_cursor_events(clickthrough)` из состояния. Без этого clickthrough-окно не получит фокус и blur-логика сломается.

**A5. Убрать A-Z-перехват из hook (для панели).**
В `hook.rs:soundpanel_keyboard_proc`:
- Ветка «A-Z → звук → гашение» удаляется (теперь это делает окно).
- Оставить только то, что нужно для Intercept (ЧАСТЬ B). Если ЧАСТЬ B в этой же итерации — refactor сразу под Intercept; если нет — временно proc может стать pass-through для A-Z (просто не возвращать LRESULT(1), а `CallNextHookEx`), чтобы не блокировать клавиши глобально.

> Решить при планировании задач: делать A и B в одной итерации или последовательно. **Рекомендация:** последовательно (сначала A — проверяем окно; потом B — Intercept). Меньше радиус изменений за один прогон.

---

### ЧАСТЬ B — Intercept (Stage 2)

**B1. Persisted-конфиг `intercept.json`.**
Новый модуль `src-tauri/src/intercept/mod.rs` (+ `state.rs`, `storage.rs` — по образцу `soundpanel/`):
- Структура:
  ```rust
  #[derive(Serialize, Deserialize, Clone, Default)]
  pub struct InterceptSettings {
      pub enabled: bool,
      pub bindings: Vec<InterceptBinding>,
  }
  #[derive(Serialize, Deserialize, Clone)]
  pub struct InterceptBinding {
      pub key: String,        // "NUMPAD1", "F5", "NUMPAD_ADD" ...
      pub action: String,     // "show_main_window" | "show_soundpanel_window" |
                              //   "show_playback_control_window" |
                              //   "playback_pause" | "playback_stop" | "playback_repeat"
  }
  ```
- `storage.rs` — load/save в `%APPDATA%/ttsbard/intercept.json` (по образцу `soundpanel/storage.rs`).
- Регистрация в app state (как `SoundPanelState`): `app.manage(InterceptState::new(...))` в `setup.rs`.

**B2. Обобщённый hook: Intercept-режим.**
В `hook.rs` (или новом `intercept/hook.rs`):
- proc проверяет **persisted-toggle** `intercept.enabled` (НЕ soundpanel runtime-флаг).
- Диапазон: NumPad + F-keys. VK-коды:
  - `VK_NUMPAD0..VK_NUMPAD9` = 0x60..0x69, `VK_MULTIPLY`=0x6A, `VK_ADD`=0x6B, `VK_SEPARATOR`=0x6C, `VK_SUBTRACT`=0x6D, `VK_DECIMAL`=0x6E, `VK_DIVIDE`=0x6F.
  - `VK_F1..VK_F24` = 0x70..0x87.
- Логика: при `WM_KEYDOWN`, если `enabled` и клавиша в диапазоне и есть биндинг → вызвать action (см. B3) + `return LRESULT(1)` (фильтруем, не отдаём в систему). Иначе → `CallNextHookEx` (проходит насквозь).
- **Не гасить** toggle после срабатывания (в отличие от старого soundpanel-hook). Toggle держится, пока пользователь не выключит.
- Решить B-развилку: один hook на оба режима или два? **Рекомендация:** единый proc с чётким switch по контексту: если `intercept.enabled` → Intercept-логика; иначе pass-through. Soundpanel-режим (A-Z) удалён в A5, так что конфликтов нет.

**B3. Actions-слой: вынести хендлеры хоткеев в переиспользуемые функции.**
В `hotkeys.rs` (или новом `actions.rs`):
- Сделать публичные функции: `run_action(app_handle, action: &str)`, который матчит строку action на существующие `handle_*` (`handle_main_window`, `handle_sound_panel`, `handle_playback_control_window`, `handle_playback_pause`, `handle_playback_stop`, `handle_playback_repeat`).
- Hotkey-регистрации переписать вызывать `run_action` (или оставить как есть, если `handle_*` уже самодостаточны — тогда Intercept просто вызывает тот же `handle_*` по строке action).
- Hook вызывает `run_action` из proc (нужен `AppHandle` — он уже доступен через `SP_HOOK_STATE`/state или новое `OnceLock<AppHandle>`).

**B4. Tauri-команды + события.**
- `get_intercept_settings` → `InterceptSettings`.
- `set_intercept_enabled(enabled: bool)` → сохраняет в `intercept.json` + `emit(InterceptionChanged(enabled))`.
- `set_intercept_binding(key, action)` / `clear_intercept_binding(key)` → обновляют bindings + сохраняют.
- Команды зарегистрировать в `lib.rs`/`commands`.
- `InterceptionChanged` → переиспользовать существующий AppEvent (events.rs) + Tauri-event на фронт.

**B5. UI: панель «Перехват» в сайдбаре.**
- Новый `src/components/InterceptPanel.vue` по образцу `HotkeysPanel.vue`:
  - Toggle «Перехват вкл/выкл» → `set_intercept_enabled`.
  - Список биндингов: клавиша → action (select из 6 actions).
  - **Запись клавиши:** кнопка «Записать» → локальный keydown-capture (как `HotkeysPanel.vue:handleKeyDown`), но фильтр только NumPad/F-keys; показывает предупреждение, если клавиша вне диапазона.
- Регистрация:
  - `Panel` type в `App.vue`/`Sidebar.vue`: добавить `'intercept'`.
  - Кнопка в `Sidebar.vue` в группе рядом с «Горячие клавиши» (ниже), иконка из `lucide-vue-next` (напр. `Magnet` / `Crosshair`).
  - `<InterceptPanel v-show="currentPanel === 'intercept'" />` в `App.vue`.

---

## 3. Что НЕ делать (в этом плане)
- Не биндить звук/TTS-фразу как action Intercept (отдельный план позже).
- Не трогать `tauri-plugin-global-shortcut` (механизм выбран — hook).
- Не менять существующие хоткеи и их дефолты.
- Не менять DTO `PlaybackStateDto`, playback-команды, позиционирование окон (`WindowsManager`).
- Не делать Intercept кросс-платформенным (Windows-only hook — приемлемо, проект Windows-first).

---

## 4. Критерии приёмки (Definition of Done)

**ЧАСТЬ A:**
1. Открытие soundpanel хоткеем → окно **в фокусе** (`set_focus`), без включения старого перехвата (`interception_enabled` не ставится в true для панели).
2. В окне: нажатие A-Z (без модификаторов) → звук (если забинжено) → окно закрылось; если не забинжено → `showNoBinding`, окно остаётся.
3. Escape → окно закрылось.
4. Клик мимо окна (blur) → окно скрылось.
5. Clickthrough-режим: если включён — снимается на активной фазе (окно получает фокус), восстанавливается после hide.
6. Глобальный hook больше **не блокирует** A-Z (клавиши проходят в систему, когда intercept-toggle выключен).
7. Drag/кнопки title-bar/transparency/clickthrough-toggle — без регрессий.

**ЧАСТЬ B:**
8. `intercept.json` создаётся в `%APPDATA%/ttsbard/`, persist между запусками.
9. Toggle «Перехват» в UI включает/выключает перехват; состояние сохраняется.
10. Забинженная NumPad/F-клавиша, когда toggle вкл, **не доходит до системы** и вызывает выбранный action (открытие окна / playback-команда). Незабинженные клавиши проходят насквозь.
11. Действия = ровно 6 (3 открытия окон + 3 playback-команды), переиспользуют `handle_*` из hotkeys.rs.
12. Панель «Перехват» в сайдбаре: toggle + запись клавиши (только NumPad/F) + выбор action + очистка биндинга.
13. Перехват работает, даже когда главное окно в фокусе (отличие от `global-shortcut`).
14. Сборка: `npx vue-tsc --noEmit` → 0 errors; `cargo check` → green.

**Known limitation (зафиксировать в UI/документации):**
15. `set_focus()` soundpanel может сбросить эксклюзивный full-screen. Не чинить — known.

---

## 5. Порядок реализации (для DeepSeek-итераций)

Рекомендация: **последовательно, ЧАСТЬ A → отдельная итерация → ЧАСТЬ B.**

- **Итерация A** (task `91-A-01.md`): A1–A5 (soundpanel как окно). Ревью → фикс.
- **Итерация B** (task `91-B-01.md`): B1–B5 (Intercept). Ревью → фикс.

Можно и одной итерацией, но радиус изменений большой (Rust hook refactor + Vue + новая панель + persisted-конфиг) — две итерации снижают риск и упрощают ревью.

---

## 6. Риски / заметки для ревью
- **Blur-надёжность:** проверить, что внутренние элементы (transparency-control) не триггерят `Focused(false)`. Если триггерят — добавить флаг «transparency-open» и не скрывать, пока он активен.
- **Clickthrough ↔ фокус:** A4 критичен — без снятия clickthrough окно не получит фокус и A2 не сработает.
- **Двойной звук:** убедиться, что после A5 hook не остаётся «зомби»-перехвата A-Z, который дублировал бы окно.
- **AppHandle в hook-proc:** для B3 нужен доступ к `AppHandle` из proc — завести `OnceLock<AppHandle>` при инициализации hook (по образцу `SP_HOOK_STATE`).
- **Эмуляция `handle_*` из proc:** действия запускают window-show через `emit(Show*Window)` по mpsc-каналу — убедиться, что proc может слать в канал (state уже хранит `event_sender`).
