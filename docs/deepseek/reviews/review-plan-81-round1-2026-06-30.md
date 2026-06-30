# Review: Plan 81 (offline spellcheck backend) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `cargo check` 0 ошибок, `cargo clippy --lib` 0 warnings (после правки derive Default).
`vue-tsc` — не тронут (фронт — план 78).

## Что ревьюено
- `src-tauri/Cargo.toml` — `spellbook = "0.4"`.
- `src-tauri/resources/dict/ru.aff` (70KB) + `ru.dic` (3.4MB) — РЕАЛЬНЫЕ словари (UTF-8 Hunspell).
- `src-tauri/src/spellcheck.rs` — `SpellcheckManager` + `SpellResult`.
- `src-tauri/src/commands/spellcheck.rs` — команда `spellcheck`.
- `src-tauri/src/setup.rs` — `init_spellcheck` (resource_dir + graceful degradation).
- `src-tauri/src/state.rs` — `spellcheck_manager` поле.
- `src-tauri/tauri.conf.json` — `bundle.resources: ["resources/dict/*"]`.
- `src-tauri/src/commands/mod.rs` + `lib.rs` — регистрация.

## Соответствие плану + spike 80
- ✅ API spellbook по spike: `Dictionary::new(aff, dic)`, `check()`, `suggest(word, &mut Vec)`.
- ✅ **Graceful degradation:** `SpellcheckManager::new` при ошибке загрузки → dict=None,
  eprintln лог; `check_words` в ветке без dict → всем `correct=true` (фича молча выключена).
  `init_spellcheck` в setup: если resource_dir недоступен или файлов нет → warn + return
  (НЕ краш приложения).
- ✅ **Borrow-checker корректен:** check_words разбит на фазы — cache read (Phase 1),
  dict read в отдельном scope (Phase 2), cache write после отпуска dict-lock (Phase 3),
  merge (Phase 4). Нет одновременного read-dict + write-cache.
- ✅ **Кэш:** `or_insert_with` — не дёргает движок повторно для известных слов.
- ✅ Словарь read-only, `spawn_save` НЕ нужен (в отличие от HistoryManager).
- ✅ Регистрация: `pub mod spellcheck`, команда в invoke_handler, AppState поле, setup init.

## MINOR (отложено в задачу #13)
- `check_words` Phase 4 переусложнён: дважды перечитывает cache и пересобирает
  `final_results: Vec<Option<_>>` + `.unwrap()`. Можно упростить — слить results (cached)
  + new_results по индексам без двойного чтения cache и без Option/unwrap. Не баг (работает
  корректно), OPTIMIZE-level.
- `SpellResult` — ручной `#[derive(serde::Serialize)]` без Deserialize (ок, только наружу).

## Clippy warning (исправлено в этом ревью)
`history.rs:341` ручной `impl Default for HistoryData` → заменён на `#[derive(Default)]`
в определении структуры (предсуществующий код плана 75, clippy подсветил при сборке 81).
Теперь `cargo clippy --lib` = 0 warnings.

## End-to-end (требует ручного запуска приложения)
После включения `spellcheck_enabled` (через settings) + `source=offline` (default) — фронт
(план 78) вызывает `invoke('spellcheck', { words })` → бэкенд проверяет через spellbook →
CodeMirror linter подсвечивает ошибки + варианты исправления. **Ручная проверка запуском**
рекомендуется (не в этом цикле).

## План 81 — РЕАЛИЗОВАН (офлайн-орфография, spellbook + Hunspell ru). Сборка чистая.
