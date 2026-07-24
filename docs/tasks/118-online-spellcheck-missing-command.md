# TASK-118 — Исправить неработающий online spellcheck

**Статус:** `planned` — frontend вызывает отсутствующую backend-команду
**Связано:** [ROADMAP-007](../roadmap/completed/007-editor-menu-ai-history-spellcheck.md),
[ROADMAP-008](../roadmap/completed/008-offline-spellcheck-hunspell-codemirror.md)

**Дата обнаружения:** 2026-07-08
**Компонент:** редактор → проверка орфографии (`src/composables/useSpellcheck.ts`)

## Симптом

В настройках редактора поддерживается выбор источника проверки орфографии
`online`/`offline` (поле `spellcheck_source`, бэкенд `SpellSource::Online|Offline`,
`src-tauri/src/config/settings.rs`). Если пользователь включает режим
**online**, проверка орфографии **молча не работает**: слова не подчёркиваются,
quick-fix варианты замены не появляются — интерфейс ведёт себя так, будто
орфография выключена, без какого-либо сообщения об ошибке.

В режиме **offline** всё работает штатно.

## Корневая причина

Фронт `useSpellcheck.ts` выбирает имя команды в зависимости от источника:

`src/composables/useSpellcheck.ts`:
```ts
async function checkWords(words: string[]): Promise<SpellResult[]> {
  if (source.value === 'off' || words.length === 0) return []
  const cmd = source.value === 'online' ? 'check_spelling_online' : 'spellcheck'
  return invoke<SpellResult[]>(cmd, { words })
}
```

Команда **`check_spelling_online` в бэкенде не существует** — никогда не была
реализована. В `invoke_handler` из `src-tauri/src/lib.rs` зарегистрирована
только офлайн-команда:
```rust
commands::spellcheck::spellcheck,
```

Самого `#[tauri::command] check_spelling_online` нет ни в
`src-tauri/src/commands/spellcheck.rs`, ни в каком-либо другом модуле
(grep по `check_spelling_online` по `src-tauri/src/` пуст).

Следовательно, при `source === 'online'` `invoke('check_spelling_online')`
падает с ошибкой «command not found» — но эта ошибка **глушится** в linter:

`src/components/editor/spellLinter.ts`:
```ts
let results: SpellResult[]
try {
  results = await checkWords(words)
} catch {
  return []   // ← ошибка проглатывается, диагностики просто не возвращаются
}
```

`return []` = «нет ошибок в тексте», что выглядит как «всё правильно», а не как
«проверка упала». Поэтому сбой невидим для пользователя.

## Почему это баг

1. **Молчаливый сбой** — пользователь включает орфографию, ничего не подчёркивается,
   никаких признаков, что фича не работает. Худший вариант UX для проверки.
2. **Мёртвая ветка кода** — онлайн-путь (`source === 'online'`) гарантированно
   падает; настройка `spellcheck_source = online` по сути бесполезна, пока не
   реализован провайдер.
3. **Бэкенд и фронт рассинхронизированы** — фронт обещает онлайн (Stage 07:
   [ROADMAP-007](../roadmap/completed/007-editor-menu-ai-history-spellcheck.md)), а бэкенд реализовал
   только офлайн ([ROADMAP-008](../roadmap/completed/008-offline-spellcheck-hunspell-codemirror.md)).

## Выбранное направление

До появления отдельного online-провайдера включённая проверка орфографии всегда
использует существующую команду `spellcheck`. Значение настройки `online`,
которое могло сохраниться в старом `settings.json`, временно трактуется как
offline и больше не вызывает несуществующую Tauri-команду.

Это небольшой совместимый фикс: он устраняет гарантированный command-not-found,
не вводит новый сетевой контракт и не требует миграции пользовательских настроек.

## Scope

- убрать выбор `check_spelling_online` из `src/composables/useSpellcheck.ts`;
- сохранить состояния `off` и включённой offline-проверки;
- добавить frontend-тесты для `offline`, legacy `online`, выключенного режима и
  пустого списка слов;
- при ошибке реальной команды не выдавать её за успешную online-проверку;
  существующая политика отображения runtime-ошибки меняется только отдельной
  задачей.

## Не входит в задачу

- реализация LanguageTool, Yandex.Speller или другого сетевого провайдера;
- добавление новой Tauri-команды и сетевого proxy/cache слоя;
- удаление `SpellSource::Online` из Rust DTO и миграция `settings.json`;
- новый переключатель источника в UI.

## Затронутые файлы

- `src/composables/useSpellcheck.ts` — выбор существующей команды;
- frontend test-файл composable — regression-сценарии источника и выключения.

## Шаги воспроизведения

1. Запустить приложение.
2. В настройках редактора (`SettingsAiPanel.vue`) включить проверку орфографии.
3. Если доступен переключатель источника — выбрать **online** (на момент записи
   UI-переключателя ещё нет, но `spellcheck_source` можно выставить в `online`
   напрямую в `settings.json`: `"spellcheck_source": "online"`).
4. Ввести в редактор слово с намеренной ошибкой (например «приывет»).
5. Наблюдать: подчёркивание **не появляется**, quick-fix недоступен — хотя
   орфография «включена». В офлайн-режиме то же слово подчёркивается.

## Критерии готовности

- при `spellcheck_enabled = true` значения `online` и `offline` вызывают только
  зарегистрированную команду `spellcheck`;
- при выключенной проверке и пустом списке слов IPC не вызывается;
- существующий offline-сценарий не изменён;
- в frontend отсутствует строка `check_spelling_online`;
- regression-тесты и TypeScript build проходят.

## Проверки

```powershell
rg -n "check_spelling_online" src src-tauri/src
npm test
npm run build
```
