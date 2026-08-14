# Дорожная карта TTSBard

Этот раздел описывает путь развития продукта. Подробные roadmap items
распределены по фактическому состоянию:

- [`active/`](./active/) — исследуемые, запланированные, выполняемые и
  отложенные направления;
- [`completed/`](./completed/) — завершённые направления с полезным outcome;
- [`rejected/`](./rejected/) — осознанно отклонённые направления с причиной и
  условиями возможного пересмотра.

## Формат roadmap item

Каждый item начинается с canonical YAML front matter:

```yaml
---
id: ROADMAP-047
status: completed
created: 2026-07-24
updated: 2026-07-25
related_tasks: []
---
```

Номер в `id` совпадает с трёхзначным префиксом имени файла. `related_tasks`
содержит только идентификаторы долговечных задач вида `TASK-NNN`.

- В `active/` допустимы `exploring`, `planned`, `in_progress` и `deferred`.
- В `completed/` допустимы `completed` и `superseded`; завершённый item содержит
  непустой раздел `Outcome`.
- В `rejected/` допустимы `rejected` и `superseded`; отклонённый item содержит
  раздел `Reconsider when`.
- `superseded` item обязательно ссылается на заменивший roadmap item или
  decision.

Формат и соответствие каталога статусу проверяет `scripts/check-docs.ps1`.

## Активные направления

| Item | Статус | Следующий шаг |
|---|---|---|
=======
| [ROADMAP-070 — маршрутизация и очередь SoundPanel](./active/070-soundpanel-audio-routing-and-queue.md) | planned | Решить, нужен ли пользователю компактный индикатор очереди SoundPanel. |
| [ROADMAP-074 — статусы интеграций в titlebar](./active/074-ambient-integration-status.md) | in_progress | Реализовать truthful gray/green/red mapping и отдельный правый status cluster перед SoundPanel/Playback. |

## Завершённые направления

### Редактор и история

- [ROADMAP-073 — маршрут фразы и результат доставки](./completed/073-readable-message-routing-and-delivery-outcomes.md)
- [ROADMAP-071 — отправка редактора и читаемый быстрый режим](./completed/071-editor-submit-and-quick-mode-affordance.md)

- [ROADMAP-068 — локальные горячие клавиши редактора](./completed/068-editor-scoped-hotkeys-and-submit-flow.md)

- [ROADMAP-001 — CodeMirror](./completed/001-monaco-vs-codemirror-editor-research.md)
- [ROADMAP-002 — локальная история и autocomplete](./completed/002-local-history-autocomplete.md)
- [ROADMAP-003 — гибридное text completion](./completed/003-text-completion-without-ai.md)
- [ROADMAP-005 — phrase history](./completed/005-phrase-history.md)
- [ROADMAP-006 — editor tabs](./completed/006-editor-tabs-multiple-texts.md)
- [ROADMAP-007 — editor menu](./completed/007-editor-menu-ai-history-spellcheck.md)
- [ROADMAP-008 — offline spellcheck](./completed/008-offline-spellcheck-hunspell-codemirror.md)
- [ROADMAP-067 — контекстное исправление орфографии](./completed/067-editor-spellcheck-context-menu.md)
- [ROADMAP-012 — persistence вкладок](./completed/012-editor-tabs-persistence.md)
- [ROADMAP-027 — layout, history и export](./completed/027-text-editor-layout-history-and-export.md)
- [ROADMAP-033 — phrase audio cache](./completed/033-phrase-history-audio-cache.md)
- [ROADMAP-034 — Silero metadata и recent dedup](./completed/034-silero-voice-and-playback-recent-dedup.md)
- [ROADMAP-048 — Enter/Escape и autocomplete](./completed/048-editor-autocomplete-enter-escape.md)
- [ROADMAP-049 — Надёжная корреляция ответов Silero](./completed/049-silero-response-correlation.md)
- [ROADMAP-052 — Перехват ошибки лимита Silero](./completed/052-silero-limit-error-handling.md)
- [ROADMAP-053 — Настройка ожидания и повторов Silero](./completed/053-silero-runtime-tuning.md)

### Окна, ввод и sound panel

- [ROADMAP-072 — удаление legacy-панели Playback](./completed/072-remove-legacy-playback-panel.md)
- [ROADMAP-004 — playback control](./completed/004-playback-control-floating-window.md)
- [ROADMAP-009 — playback window architecture](./completed/009-playback-window-architecture-analysis.md)
- [ROADMAP-010 — playback window settings](./completed/010-playback-window-settings-analysis.md)
- [ROADMAP-011 — keyboard input redesign](./completed/011-keyboard-input-mechanism-redesign.md)
- [ROADMAP-014 — soundpanel sets](./completed/014-soundpanel-sets-and-inline-editing.md)
- [ROADMAP-023 — transparency и appearance](./completed/023-window-transparency-and-unified-appearance.md)
- [ROADMAP-026 — compact appearance](./completed/026-main-window-compact-appearance.md)
- [ROADMAP-037 — Hotkeys и возврат фокуса](./completed/037-application-hotkeys-and-previous-window-focus.md)
- [ROADMAP-051 — вызов плавающих окон мышью](./completed/051-mouse-access-to-floating-windows.md)
- [ROADMAP-064 — интерактивный постоянный режим SoundPanel](./completed/064-interactive-persistent-soundpanel.md)
- [ROADMAP-065 — Саундпанель: раскладка клавиатуры, разделение runtime/config](./completed/065-soundpanel-keyboard-layout-runtime-config.md)

### Audio и playback pipeline

- [ROADMAP-020 — DeepFilterNet](./completed/020-audio-cleaning-enhancement.md)
- [ROADMAP-021 — resampling optimization](./completed/021-audio-pipeline-resampling-optimization.md)
- [ROADMAP-022 — effects navigation и preview](./completed/022-audio-effects-navigation-and-preview.md)
- [ROADMAP-024 — Signalsmith Stretch](./completed/024-signalsmith-stretch-audio-effects.md)
- [ROADMAP-025 — PCM playback pipeline](./completed/025-playback-pcm-pipeline.md)
- [ROADMAP-029 — DSP postprocessing](./completed/029-dsp-audio-postprocessing.md)
- [ROADMAP-030 — sample-rate invariant](./completed/030-deepfilternet-resampling-invariant.md)
- [ROADMAP-031 — Resemble Enhance research](./completed/031-resemble-enhance-research.md)
- [ROADMAP-032 — audio boundaries и presets](./completed/032-audio-boundaries-and-dsp-presets.md)
- [ROADMAP-035 — AudioPanel decomposition](./completed/035-audio-panel-subpanels.md)
- [ROADMAP-066 — Переработка UI эффектов и DSP](./completed/066-audio-effects-dsp-ui-redesign.md)
- [ROADMAP-047 — очередь задач озвучивания](./completed/047-speech-job-queue.md)
- [ROADMAP-050 — Единый список управления воспроизведением](./completed/050-unified-playback-activity-list.md)

### Архитектура, AI и документация

- [ROADMAP-015 — AI feature map](./completed/015-ai-features-map-and-token-benchmark.md)
- [ROADMAP-016 — project repositioning](./completed/016-project-repositioning.md)
- [ROADMAP-017 — documentation и presentation](./completed/017-documentation-and-streamer-presentation.md)
- [ROADMAP-018 — runtime architecture и AppState](./completed/018-runtime-architecture-and-appstate.md)
- [ROADMAP-019 — custom AI provider](./completed/019-custom-ai-provider.md)
- [ROADMAP-028 — secret-safe logging](./completed/028-secret-safe-logging.md)
- [ROADMAP-036 — Telegram auth polish](./completed/036-telegram-auth-flow-polish.md)
- [ROADMAP-038 — Dynamic Piper providers](./completed/038-dynamic-piper-tts-providers.md)
- [ROADMAP-039 — Embedded Piper runtime и лицензирование](./completed/039-piper-runtime-feasibility.md)
- [ROADMAP-040 — Test coverage gaps](./completed/040-test-coverage-gaps.md)
- [ROADMAP-041 — Review 021 remediation](./completed/041-review-021-remediation.md)
- [ROADMAP-046 — Documentation migration](./completed/046-documentation-structure-migration.md)
- [ROADMAP-055 — Качество и AI-ready foundation](./completed/055-quality-and-ai-ready-architecture.md)
- [ROADMAP-056 — IPC-контракты и AI-ready границы](./completed/056-ipc-contracts-and-ai-ready-boundaries.md)
- [ROADMAP-057 — Достоверный handoff и продуктовые источники](./completed/057-truthful-handoff-and-product-sources.md)
- [ROADMAP-058 — Исполнимые контракты и lifecycle-проверки](./completed/058-executable-contracts-and-lifecycle-proof.md)
- [ROADMAP-059 — Владение integration state и атомарность settings](./completed/059-integration-state-ownership-and-settings-atomicity.md) — P0–P3: WebView atomic save, Telegram owner API, контракты DECISION-018/019.

### VTube Studio и WebView

- [ROADMAP-042 — VTube Studio typing UI](./completed/042-vtube-studio-typing-ui.md)
- [ROADMAP-043 — WebView typing events](./completed/043-webview-editor-typing-events.md)
- [ROADMAP-044 — VTube Studio connection lifecycle](./completed/044-vtube-studio-connection-lifecycle-ui.md)
- [ROADMAP-045 — typing output modes](./completed/045-vtube-studio-typing-output-modes.md)
- [ROADMAP-054 — Детерминированная видимость предмета VTube Studio](./completed/054-vtube-studio-item-visibility.md)
- [ROADMAP-060 — Корректный lifecycle параметра печати VTube Studio](./completed/060-vtube-studio-parameter-input-lifecycle.md)
- [ROADMAP-061 — Настройка действия и замена custom INPUT VTube Studio](./completed/061-vtube-studio-action-configuration-and-parameter-replacement.md)
- [ROADMAP-062 — Достоверное runtime-состояние соединения VTube Studio](./completed/062-vtube-studio-runtime-connection-truth.md)
- [ROADMAP-063 — Фактическое runtime-состояние WebView-сервера](./completed/063-webview-runtime-server-status.md)

### Remediation и устойчивость

- [ROADMAP-069 — Устранение дефектов полного review 0.21.0](./completed/069-full-review-correctness-remediation.md)

## Отклонённые направления

- [ROADMAP-013 — переход history storage на SQLite](./rejected/013-history-storage-json-vs-sqlite.md) — отклонён до появления объёма и конкурентного доступа, оправдывающих БД.
