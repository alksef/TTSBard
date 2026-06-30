# Review: Plan 78 (spell-linter layer) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `vue-tsc` 0 ошибок, `cargo check` 0 ошибок.

## Что ревьюено
- НОВЫЙ `src/types/spell.ts` — `SpellResult` интерфейс.
- НОВЫЙ `src/composables/useSpellcheck.ts` — composable, source online/offline/off, checkWords.
- НОВЫЙ `src/components/editor/spellLinter.ts` — `createSpellLinter`, токенизация `[a-zа-яё]+` с флагом `u`, debounce 400ms, catch→[].
- ИЗМЕНЁН `src/components/editor/TtsEditor.vue` — linter в extensions + тема `.cm-lintRange`/`.cm-diagnosticText` через `--color-danger`.
- ИЗМЕНЁН `src-tauri/src/config/settings.rs` — `SpellSource` enum (default Offline) + поля `spellcheck_enabled`/`spellcheck_source` с `#[serde(default)]` + геттеры/сеттеры.
- ИЗМЕНЁН `src-tauri/src/commands/mod.rs` + `lib.rs` — 4 команды set/get, зарегистрированы.
- `package.json` — `@codemirror/lint` установлен.

## Соответствие плану
- ✅ Каркас источника-агностичен: `checkWords` выбирает команду по source.
- ✅ Токенизация рус+лат с флагом `u` (кириллица ловится).
- ✅ Debounce `{ delay: 400 }`.
- ✅ `#[serde(default)]` на новых полях + Default на SpellSource → миграция старых конфигов безопасна (урок playback_pause исключён).
- ✅ Тема linter через CSS-vars (light/dark).
- ✅ При отсутствии бэкенд-команды `spellcheck` catch→[] (нет крашей) — каркас внедрён безопасно.

## Поведение сейчас
- `spellcheck_enabled = false` (default) → linter молчит.
- Если включить → `checkWords` вызывает несуществующую `spellcheck` → IPC ошибка → catch → `[]` → без крашей.
- Реальная подсветка появится после плана 81 (команда `spellcheck` + spellbook).

## Замечаний нет. План 78 — базовый слой готов.
**После планов 79/81** — повторное ревью (особенно end-to-end: реальная подсветка с spellbook).
