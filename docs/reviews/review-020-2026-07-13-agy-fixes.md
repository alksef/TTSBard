# Review 020 — исправления

Дата: 2026-07-13  
Исходный review: `review-020-2026-07-13-agy.md`

## Выполнено отдельными коммитами

- `f2edd38` — coalescing перезагрузки settings и типизация `invoke`.
- `289c5b6` — единая публикация `settings-changed` после persisted setters.
- `35e8644` — общий persistence primitive, atomic JSON write, lock/cache для окон.
- `7dd344d` — общий cache settings на TTS hot path.
- `63eef5d` — persist settings перед применением runtime TTS state.
- `634312d`, `1a9808a`, `62fd0b9` — перенос синхронных config writes с command/UI thread в `spawn_blocking`.
- `df49b6d` — recovery повреждённых `settings.json`/`windows.json` с backup и defaults.
- `3fd430e` — управляемый lifecycle Windows keyboard hook: WM_QUIT, join и unhook.
- `92323e8` — HotkeysPanel и AudioPanel используют typed unified settings read-model.
- `0fa6ce1` — централизованные имена события `settings-changed` в backend/frontend.

## Проверки

- `cargo check` — успешно.
- `npx vue-tsc --noEmit` — успешно.
- config/settings tests — успешно; добавлены проверки concurrent updates, corrupted JSON и persist failure.
- Полный `cargo test --lib` выполняется с одним воспроизводимым ранее падением `signalsmith::wrapper::tests::test_repeated_calls` (`StretchError`, code `-3`); изменения review не затрагивают этот модуль.

## Не входит в этот цикл

Security замечание по unified DTO/API keys намеренно не менялось и требует отдельного обсуждения.

Крупные архитектурные предложения из раздела OPTIMIZE — полноценный settings service, versioned typed event payload и end-to-end IPC tests — не смешаны с исправлениями minor и требуют отдельного проектного решения.
