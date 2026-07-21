# TASK-118 — Исправить неработающий online spellcheck

**Статус:** `planned` — frontend вызывает отсутствующую backend-команду
**Связано:** [ROADMAP-007](../roadmap/completed/007-editor-menu-ai-history-spellcheck.md),
[ROADMAP-008](../roadmap/completed/008-offline-spellcheck-hunspell-codemirror.md)

**Дата обнаружения:** 2026-07-08
**Компонент:** редактор → проверка орфографии (`src/composables/useSpellcheck.ts`)

## Симптом

В настройках редактора поддерживается выбор источника проверки орфографии
`online`/`offline` (поле `spellcheck_source`, бэкенд `SpellSource::Online|Offline`,
`src-tauri/src/config/settings.rs:435-441`). Если пользователь включает режим
**online**, проверка орфографии **молча не работает**: слова не подчёркиваются,
quick-fix варианты замены не появляются — интерфейс ведёт себя так, будто
орфография выключена, без какого-либо сообщения об ошибке.

В режиме **offline** всё работает штатно.

## Корневая причина

Фронт `useSpellcheck.ts` выбирает имя команды в зависимости от источника:

`src/composables/useSpellcheck.ts:16-20`:
```ts
async function checkWords(words: string[]): Promise<SpellResult[]> {
  if (source.value === 'off' || words.length === 0) return []
  const cmd = source.value === 'online' ? 'check_spelling_online' : 'spellcheck'
  return invoke<SpellResult[]>(cmd, { words })
}
```

Команда **`check_spelling_online` в бэкенде не существует** — никогда не была
реализована. В `invoke_handler` зарегистрирована только офлайн-команда:

`src-tauri/src/lib.rs:451`:
```rust
commands::spellcheck::spellcheck,
```

Самого `#[tauri::command] check_spelling_online` нет ни в
`src-tauri/src/commands/spellcheck.rs`, ни в каком-либо другом модуле
(grep по `check_spelling_online` по `src-tauri/src/` пуст).

Следовательно, при `source === 'online'` `invoke('check_spelling_online')`
падает с ошибкой «command not found» — но эта ошибка **глушится** в linter:

`src/components/editor/spellLinter.ts:24-29`:
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

## Возможные подходы к исправлению (для плана)

Решить в плане DeepSeek — выбрать один:

- **A. Реализовать онлайн-провайдер (полноценный фикс).** Добавить
  `#[tauri::command] check_spelling_online(words) -> Vec<SpellResult>`: HTTP-запрос
  к LanguageTool или Yandex.Speller, с учётом сетевого прокси (по образцу
  OpenAI/AI-запросов, `/tts/network/proxy`) и кэшем. Так описано в Stage 07.
  Это «средняя/крупная» задача (HTTP + прокси + кэш + парсинг ответа).
- **B. Убрать онлайн-режим из UI, пока провайдер не реализован.** Не давать
  пользователю выбрать `online` (только off/offline), либо если выбран — трактовать
  как off с предупреждением. Быстрый фикс, откладывает полноценную реализацию.
- **C. Гибрид:** реализовать B сейчас (честное поведение) и завести отдельный
  план на A.

Рекомендация — **B или C**: пока онлайн-провайдера нет, режим `online` не должен
тихо падать. Полноценный провайдер (A) — отдельная задача со своим планом.

> Связано: при добавлении UI-переключателя online/offline
> (`SettingsAiPanel.vue`) важно учесть этот баг — переключатель не должен
> предлагать нерабочий `online` без оговорки.

## Затронутые файлы

- `src/composables/useSpellcheck.ts:16-20` — выбор имени команды по `source`.
- `src/components/editor/spellLinter.ts:24-29` — `catch { return [] }` глушит сбой.
- `src-tauri/src/lib.rs:451` — `invoke_handler` (есть только `spellcheck`).
- `src-tauri/src/commands/spellcheck.rs` — отсутствует `check_spelling_online`.
- `src-tauri/src/config/settings.rs:435-456` — `SpellSource::Online|Offline`,
  поле `spellcheck_source` (настройка живёт, команды для онлайн — нет).

## Шаги воспроизведения

1. Запустить приложение.
2. В настройках редактора (`SettingsAiPanel.vue`) включить проверку орфографии.
3. Если доступен переключатель источника — выбрать **online** (на момент записи
   UI-переключателя ещё нет, но `spellcheck_source` можно выставить в `online`
   напрямую в `settings.json`: `"spellcheck_source": "online"`).
4. Ввести в редактор слово с намеренной ошибкой (например «приывет»).
5. Наблюдать: подчёркивание **не появляется**, quick-fix недоступен — хотя
   орфография «включена». В офлайн-режиме то же слово подчёркивается.
