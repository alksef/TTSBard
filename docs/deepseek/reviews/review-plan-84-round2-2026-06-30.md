# Review: Plan 84 round2 — доработки окна управления (после runtime-теста)

**Дата:** 2026-06-30
**Verdict:** PARTIAL — баги 1,2 починены; баг 3 требует runtime-проверки (DeepSeek сделал
обходной путь, но не диагностировал корень).
**Сборка:** `vue-tsc` 0, `cargo check` 0, `clippy` 0 warnings.

## Баг 1 (хоткей в панели) — ✅ ПОЧИНЕНО
- `HotkeysPanel.vue`: импорт `MonitorPlay`, тип `recordingFor` расширен `'playback_control_window'`,
  `startRecording` принимает новое имя, case в `saveHotkey`/`resetToDefault`.
- Шаблон: строка «Управление воспроизведением» с иконкой MonitorPlay + кнопками Изменить/Сброс.
- `types/settings.ts:26` — `playback_control_window: HotkeyDto` в HotkeySettingsDto (есть).

## Баг 2 (drag + close) — ✅ ПОЧИНЕНО
- `PlaybackControlApp.vue`: кнопка `<button class="close-btn">✕</button>` в header,
  `closeWindow()` → `getCurrentWindow().hide()` (скрытие, не destroy — окно переиспользуется).
- CSS `.close-btn` (CSS-vars, hover danger).
- Drag: `data-tauri-drag-region` на header остался; close-btn/status-badge по умолчанию не
  блокируют drag остальной области.

## Баг 3 (нет реакции на воспроизведение) — ⚠️ PARTIAL / ТРЕБУЕТ RUNTIME-ПРОВЕРКИ
- DeepSeek добавил **обходной путь**: `playback_window.rs` эмитит `window.emit("refresh-state", ())`
  после show → `PlaybackControlApp.vue` слушает `refresh-state` → `fetchState()`.
- Это обновит состояние **один раз при открытии окна**, но НЕ гарантирует live-обновление
  во время воспроизведения (когда окно уже открыто и приходит `playback-started`/`queue-changed`).
- **DeepSeek НЕ диагностировал**, доходят ли `app.emit("playback-started"/"queue-changed")`
  до webview playback-control. Бэкенд эмитит (playback.rs:189-293), фронт подписан
  (PlaybackControlApp:53-60). Возможные причины, если не доходят: Tauri 2 + скрытое окно
  не обрабатывает события; или emit до mount listener.

### Что проверить runtime
1. Открыть окно хоткеем Ctrl+Shift+F7 → статус/очередь корректны (refresh-state сработал). ✅ (ожидаемо)
2. **При открытом окне** отправить текст на TTS → должно прийти `queue-changed` + `playback-started`
   → статус обновится live. Если НЕ обновляется → баг доставки событий, отдельный план.
3. История фраз (PhraseHistoryList в главном окне) — НЕ обновляется реактивно после speak
   (грузит список при развороте, без подписки на событие). Отдельный UX-баг.

## Статус
- Баги 1, 2 — готовы, коммитить.
- Баг 3 — refresh-state добавлен (минимальный фикс для «при открытии видно актуальное»);
  live-обновление требует runtime-проверки. Если не работает — план 85 (доставка событий
  в окно playback-control: emit_to / подписка / polling).
