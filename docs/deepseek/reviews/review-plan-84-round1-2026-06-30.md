# Review: Plan 84 (playback-control window show) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `cargo check` 0 ошибок, `cargo clippy --lib` 0 warnings, `vue-tsc` 0 ошибок.

## Что ревьюено
- НОВЫЙ `src-tauri/src/playback_window.rs` — show/hide по образцу soundpanel.
- `src-tauri/src/events.rs` — ShowPlaybackControlWindow/HidePlaybackControlWindow.
- `src-tauri/src/setup.rs` — обработчики событий в event-loop thread + автозапуск по настройке
  + удалён старый exclude-capture блок (унифицирован в show-функции).
- `src-tauri/src/hotkeys.rs` + `config/hotkeys.rs` — полноценный хоткей playback_control_window
  (Ctrl+Shift+F7) через существующую систему (default + migration-чек + reset).
- `src-tauri/src/config/settings.rs` — `show_playback_on_start: bool` (`#[serde(default)]`,
  default false) + get/set.
- `commands/mod.rs` + `lib.rs` — команда toggle (если добавлена).
- `src/components/settings/SettingsGeneral.vue` + `types/settings.ts` — чекбокс в общих
  настройках.

## Соответствие плану + UX (уточнённому с пользователем)
- ✅ **Show-функция** по образцу soundpanel: show → set_focus → set_window_exclude_from_capture
  **после** show. Это ключевое — применение capture/темы после show устраняет белый квадрат.
- ✅ **Хоткей** — DeepSeek сделал ЛУЧШЕ, чем просили: не отдельный регистратор, а через
  существующую систему хоткеев (default Ctrl+Shift+F7, migration-чек на пустой ключ,
  reset-to-default). Переиспользование, не дублирование.
- ✅ **Настройка** `show_playback_on_start` в ОБЩИХ настройках (не editor), `#[serde(default)]`,
  default false + UI чекбокс в SettingsGeneral.
- ✅ **Автозапуск** в setup.rs: `if settings.show_playback_on_start { show_playback_window() }`.
- ✅ Окно `visible: false` в конфиге оставлено — НЕ показывается само (решает «само всплывает
  белым квадратом»).
- ✅ Старый exclude-capture блок удалён (унифицирован в show-функции).

## Поведение
- По умолчанию (`show_playback_on_start = false`): окно скрыто. Открывается хоткеем
  Ctrl+Shift+F7 → show-функция → корректный UI (тема + прозрачность, не белый квадрат).
- Висит пока не закрыть крестиком (НЕ auto-hide — обычное окно управления).
- Если `show_playback_on_start = true` — показывается при старте.

## Runtime-проверка
Требует запуска приложения: открыть хоткеем → убедиться, что белый квадрат ушел (корректная
тема + прозрачность). Код-ревью подтверждает, что show-функция идентична soundpanel-образцу
(который работает). Если белый квадрат останется — DevTools диагностика Vue/CSS.

## План 84 — РЕАЛИЗОВАН. Сборка чистая (0/0/0). Готов к коммиту + runtime-проверке.
