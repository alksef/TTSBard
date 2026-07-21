# Дорожная карта TTSBard

Этот раздел описывает путь развития продукта. Подробные roadmap items
распределены по фактическому состоянию:

- [`active/`](./active/) — исследуемые, запланированные, выполняемые и
  отложенные направления;
- [`completed/`](./completed/) — завершённые направления с полезным outcome;
- [`rejected/`](./rejected/) — осознанно отклонённые направления с причиной и
  условиями возможного пересмотра.

## Активные направления

| Item | Статус | Следующий шаг |
|---|---|---|
| [ROADMAP-031 — Resemble Enhance](./active/031-resemble-enhance-research.md) | `deferred` | Вернуться после DSP A/B и отдельного proof-of-concept |
| [ROADMAP-037 — Hotkeys и возврат фокуса](./active/037-application-hotkeys-and-previous-window-focus.md) | `in_progress` | Ручная проверка runtime-сценариев |
| [ROADMAP-038 — Dynamic Piper providers](./active/038-dynamic-piper-tts-providers.md) | `in_progress` | Ручная проверка на Windows |
| [ROADMAP-039 — Embedded Piper runtime](./active/039-piper-runtime-feasibility.md) | `exploring` | Решить лицензирование phonemizer |
| [ROADMAP-040 — Test coverage gaps](./active/040-test-coverage-gaps.md) | `in_progress` | P1/P2: composables, playback seam, HTTP contracts |
| [ROADMAP-041 — Review 021 remediation](./active/041-review-021-remediation.md) | `in_progress` | Сверить оставшийся scope и закрыть item |
| [ROADMAP-046 — Documentation migration](./active/046-documentation-structure-migration.md) | `in_progress` | Выполнить этапы 2–7 |

## Завершённые направления

### Редактор и история

- [ROADMAP-001 — CodeMirror](./completed/001-monaco-vs-codemirror-editor-research.md)
- [ROADMAP-002 — локальная история и autocomplete](./completed/002-local-history-autocomplete.md)
- [ROADMAP-003 — гибридное text completion](./completed/003-text-completion-without-ai.md)
- [ROADMAP-005 — phrase history](./completed/005-phrase-history.md)
- [ROADMAP-006 — editor tabs](./completed/006-editor-tabs-multiple-texts.md)
- [ROADMAP-007 — editor menu](./completed/007-editor-menu-ai-history-spellcheck.md)
- [ROADMAP-008 — offline spellcheck](./completed/008-offline-spellcheck-hunspell-codemirror.md)
- [ROADMAP-012 — persistence вкладок](./completed/012-editor-tabs-persistence.md)
- [ROADMAP-027 — layout, history и export](./completed/027-text-editor-layout-history-and-export.md)
- [ROADMAP-033 — phrase audio cache](./completed/033-phrase-history-audio-cache.md)
- [ROADMAP-034 — Silero metadata и recent dedup](./completed/034-silero-voice-and-playback-recent-dedup.md)

### Окна, ввод и sound panel

- [ROADMAP-004 — playback control](./completed/004-playback-control-floating-window.md)
- [ROADMAP-009 — playback window architecture](./completed/009-playback-window-architecture-analysis.md)
- [ROADMAP-010 — playback window settings](./completed/010-playback-window-settings-analysis.md)
- [ROADMAP-011 — keyboard input redesign](./completed/011-keyboard-input-mechanism-redesign.md)
- [ROADMAP-014 — soundpanel sets](./completed/014-soundpanel-sets-and-inline-editing.md)
- [ROADMAP-023 — transparency и appearance](./completed/023-window-transparency-and-unified-appearance.md)
- [ROADMAP-026 — compact appearance](./completed/026-main-window-compact-appearance.md)

### Audio и playback pipeline

- [ROADMAP-020 — DeepFilterNet](./completed/020-audio-cleaning-enhancement.md)
- [ROADMAP-021 — resampling optimization](./completed/021-audio-pipeline-resampling-optimization.md)
- [ROADMAP-022 — effects navigation и preview](./completed/022-audio-effects-navigation-and-preview.md)
- [ROADMAP-024 — Signalsmith Stretch](./completed/024-signalsmith-stretch-audio-effects.md)
- [ROADMAP-025 — PCM playback pipeline](./completed/025-playback-pcm-pipeline.md)
- [ROADMAP-029 — DSP postprocessing](./completed/029-dsp-audio-postprocessing.md)
- [ROADMAP-030 — sample-rate invariant](./completed/030-deepfilternet-resampling-invariant.md)
- [ROADMAP-032 — audio boundaries и presets](./completed/032-audio-boundaries-and-dsp-presets.md)
- [ROADMAP-035 — AudioPanel decomposition](./completed/035-audio-panel-subpanels.md)

### Архитектура, AI и документация

- [ROADMAP-015 — AI feature map](./completed/015-ai-features-map-and-token-benchmark.md)
- [ROADMAP-016 — project repositioning](./completed/016-project-repositioning.md)
- [ROADMAP-017 — documentation и presentation](./completed/017-documentation-and-streamer-presentation.md)
- [ROADMAP-018 — runtime architecture и AppState](./completed/018-runtime-architecture-and-appstate.md)
- [ROADMAP-019 — custom AI provider](./completed/019-custom-ai-provider.md)
- [ROADMAP-028 — secret-safe logging](./completed/028-secret-safe-logging.md)
- [ROADMAP-036 — Telegram auth polish](./completed/036-telegram-auth-flow-polish.md)

### VTube Studio и WebView

- [ROADMAP-042 — VTube Studio typing UI](./completed/042-vtube-studio-typing-ui.md)
- [ROADMAP-043 — WebView typing events](./completed/043-webview-editor-typing-events.md)
- [ROADMAP-044 — VTube Studio connection lifecycle](./completed/044-vtube-studio-connection-lifecycle-ui.md)
- [ROADMAP-045 — typing output modes](./completed/045-vtube-studio-typing-output-modes.md)

## Отклонённые направления

- [ROADMAP-013 — переход history storage на SQLite](./rejected/013-history-storage-json-vs-sqlite.md) — отклонён до появления объёма и конкурентного доступа, оправдывающих БД.
