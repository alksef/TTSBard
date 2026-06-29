# Plan 78: Общий spellcheck-linter-слой (база для онлайн/офлайн орфографии)

**Дата:** 2026-06-30
**Статус:** draft (для реализации через DeepSeek по WORKFLOW)
**Связано:** арх-ревью `docs/reviews/review-019-2026-06-30.md` (критическое замечание:
07↔08 linter-дублирование), stage `07`, stage `08`, план 71 (CodeMirror).

## Контекст
Stage 07 (онлайн-орфография) и stage 08 (офлайн-орфография) обе сводятся к подсветке ошибок
и вариантам исправления в CodeMirror через `@codemirror/lint` (`Diagnostic { from, to,
message, severity, actions[] }`). Делать их двумя независимыми планами = **дублирование
linter-интеграции** (токенизация, debounce, маппинг результатов → Diagnostic, тема).

Этот план — **вынести общий слой ДО** реализации 07/08. Источник данных (онлайн/офлайн)
переключаемый, linter-логика одна.

> План НЕ включает сам движок проверки (онлайн LanguageTool или офлайн spellbook) — только
> **интеграционный каркас**: composable-интерфейс + linter-расширение + токенизация + тема +
> настройка вкл/выкл. Конкретный источник подключается в планах 07 (онлайн) / 08 (офлайн)
> через реализацию интерфейса `SpellSource`.

## Цели
1. Единый linter-слой в `TtsEditor.vue`, не дублируемый между 07/08.
2. Переключаемый источник: `online` / `offline` / `off`.
3. Настройка вкл/выкл в `EditorSettings` с `#[serde(default)]` (урок `playback_pause`).
4. Тема linter-декораций через CSS-vars (light/dark).
5. Debounce — не дёргать проверку на каждое нажатие.

## Точные точки интеграции (из research)
- `TtsEditor.vue:275-300` — `createState()`, массив `extensions`. Добавить `linter(...)` сюда.
- `package.json:13-18` — `@codemirror/*` версии 6.x. **`@codemirror/lint` ОТСУТСТВУЕТ** →
  `npm i @codemirror/lint`.
- `src-tauri/src/config/settings.rs:434-444` — `EditorSettings` УЖЕ имеет `#[serde(default)]`
  на структуре и полях (`quick`, `ai`, `ai_completion`). Добавить поле — тривиально, БЕЗ риска
  миграции (Default уже есть).
- `settings.rs:1086-1115` — образец геттеров/сеттеров (`set_editor_ai_completion`/
  `get_editor_ai_completion`). Добавить пару для spellcheck.
- `src/composables/useAppSettings.ts:266-269` — `useEditorSettings()` composable (фронт-доступ
  к `settings.editor`).

## Архитектура

### 1. Зависимость
`npm i @codemirror/lint` (версия 6.x, совместимая с существующими @codemirror/* 6.x).

### 2. Тип результата проверки (общий для источников)
`src/types/spell.ts` (НОВЫЙ):
```ts
export interface SpellResult {
  word: string
  correct: boolean
  suggestions: string[]
}
```
Онлайн (07) и офлайн (08) бэкенд-команды возвращают один и тот же тип → фронт не зависит от
источника.

### 3. Composable `src/composables/useSpellcheck.ts` (НОВЫЙ)
```ts
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useEditorSettings } from './useAppSettings'
import type { SpellResult } from '../types/spell'

export type SpellSource = 'online' | 'offline' | 'off'

export function useSpellcheck() {
  const editorSettings = useEditorSettings()
  // Источник выбирается настройкой (этап 1: только 'off' | 'offline'; 'online' — план 07).
  const source = computed<SpellSource>(() => {
    if (!editorSettings.value?.spellcheck_enabled) return 'off'
    // По умолчанию offline (план 08); online добавится в плане 07.
    return editorSettings.value?.spellcheck_source === 'online' ? 'online' : 'offline'
  })

  const enabled = computed(() => source.value !== 'off')

  // Фронт разбивает текст на токены; бэкенд проверяет массив слов.
  // (Токенизация рус+лат — см. linter-источник ниже, тут только IPC.)
  async function checkWords(words: string[]): Promise<SpellResult[]> {
    if (source.value === 'off' || words.length === 0) return []
    const cmd = source.value === 'online' ? 'check_spelling_online' : 'spellcheck'
    return invoke<SpellResult[]>(cmd, { words })
  }

  return { source, enabled, checkWords }
}
```
> Имена бэкенд-команд (`spellcheck` для офлайн, `check_spelling_online` для онлайн) —
> **контракт**. План 08 реализует `spellcheck`, план 07 — `check_spelling_online`. Пока их
> нет, `checkWords` будет падать в `catch` linter-источника (см. ниже) → подсветка молчит,
> без крашей. Это позволяет внедрить каркас сейчас, не дожидаясь движков.

### 4. Linter-источник + токенизация — расширение в `TtsEditor.vue`
Вынести в `src/components/editor/spellLinter.ts` (НОВЫЙ, чистая функция — тестируемая):
```ts
import { linter, type Diagnostic } from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'
import type { SpellResult } from '../../types/spell'

// Токенизация рус+лат. \w не ловит кириллицу в JS без флага u → явно добавляем а-яё.
// Флаг u обязателен для корректной работы \w с unicode и для индексов matchAll.
const WORD_RE = /[a-zа-яё][a-zа-яё-]*/giu

export interface SpellCheckFn {
  (words: string[]): Promise<SpellResult[]>
}

export function createSpellLinter(checkWords: SpellCheckFn, enabled: () => boolean) {
  return linter(async (view: EditorView): Promise<Diagnostic[]> => {
    if (!enabled()) return []
    const doc = view.state.doc.toString()
    const tokens = [...doc.matchAll(WORD_RE)] // { 0: word, index }
    if (tokens.length === 0) return []
    const words = tokens.map(t => t[0])
    let results: SpellResult[]
    try {
      results = await checkWords(words)
    } catch {
      // Бэкенд-команда ещё не реализована / недоступна → молча, без крашей.
      return []
    }
    // Маппинг результатов на Diagnostic с позициями.
    return results
      .filter(r => !r.correct)
      .map(r => {
        const m = tokens.find(t => t[0].toLowerCase() === r.word.toLowerCase())
        if (!m || m.index == null) return null
        const from = m.index
        const to = from + m[0].length
        return {
          from,
          to,
          severity: 'warning' as const,
          message: `«${m[0]}» — нет в словаре`,
          actions: r.suggestions.slice(0, 5).map(s => ({
            name: s,
            apply: (v: EditorView, f: number, t: number) =>
              v.dispatch({ changes: { from: f, to: t, insert: s } }),
          })),
        }
      })
      .filter((d): d is Diagnostic => d !== null)
  }, { delay: 400 }) // debounce — не на каждое нажатие
}
```

### 5. Подключение в `TtsEditor.vue:createState()`
```ts
import { createSpellLinter } from './spellLinter'
import { useSpellcheck } from '../../composables/useSpellcheck'

const { checkWords, enabled } = useSpellcheck()
const spellLinter = createSpellLinter(checkWords, () => enabled.value)

// в массив extensions createState():
extensions: [
  ttsTheme,
  // ...существующие...
  spellLinter, // ← добавить
],
```

### 6. Настройка (Rust, `#[serde(default)]`)
`src-tauri/src/config/settings.rs:434-444` — добавить поля:
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct EditorSettings {
    #[serde(default)]
    pub quick: bool,
    #[serde(default)]
    pub ai: bool,
    #[serde(default)]
    pub ai_completion: bool,
    #[serde(default)]
    pub spellcheck_enabled: bool,                            // ← НОВОЕ
    #[serde(default)]
    pub spellcheck_source: SpellSource,                      // ← НОВОЕ (default Offline)
}
```
`SpellSource` enum (default = `Offline`):
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpellSource {
    Online,
    #[default]
    Offline,
}
```
Геттер/сеттер по образцу `set_editor_ai_completion`/`get_editor_ai_completion`
(`settings.rs:1086-1115`).
> **Важно:** `#[serde(default)]` на структуре уже есть — миграция старых конфигов БЕЗ этих
> полей пройдет как default (false/Offline). Грабли `playback_pause` исключены.

### 7. Тема linter-декораций (CSS-vars)
В `TtsEditor.vue` тема или глобальный CSS — стилизовать `.cm-lintRange`, `.cm-diagnosticText`
через существующие CSS-vars (`--color-danger` для подчёркивания ошибок, `--color-bg-elevated`
для tooltip-фона). Light/dark работают автоматически.

## Риски и решения
1. **Бэкенд-команд пока нет** — `checkWords` падает в catch, linter молчит. Каркас
   внедряется безопасно. ✅
2. **Токенизация рус+лат** — regex с флагом `u` и явным `а-яё`. Проверить в spike/тесте, что
   кириллица ловится. (Блокирующая проверка для плана 08, но не для каркаса.)
3. **Ложные срабатывания** — имена, `%username`, `\replace`. **Решение (этап 2 / план 08):**
   исключать токены после препроцессорных подстановок или по паттерну. В этот план НЕ входит
   (каркас только); зафиксировать как известное.
4. **Конфликт с autocomplete (план 73)** — разные расширения, не мешают. Visually — цвета
   подчёркивания vs попапа не должны сливаться. Проверить тему.
5. **Debounce** — `{ delay: 400 }` в `linter()`. + бэкенд-кэш слов (в плане 08).

## Критерии готовности
- `npm i @codemirror/lint` установлен.
- `src/types/spell.ts`, `src/composables/useSpellcheck.ts`, `src/components/editor/spellLinter.ts`
  созданы.
- `TtsEditor.vue:createState()` подключает `spellLinter`.
- `EditorSettings` + `SpellSource` + геттер/сеттер в Rust с `#[serde(default)]`.
- При `spellcheck_enabled = false` linter ничего не делает. При `true` — вызывает бэкенд
  (пока падает в catch, молчит, без крашей).
- Тема light/dark для linter-декораций.
- `npx vue-tsc --noEmit` и `cargo check` — 0 ошибок, 0 warnings.

## Объём
Средний, **двухслойный** (фронт CodeMirror + Rust-настройка). Бэкенд-движок НЕ входит
(планы 07/08). По WORKFLOW — через DeepSeek.

## Зависимости / порядок
- **Этот план → план 07 (онлайн) и план 08 (офлайн).** Без него 07/08 дублируют linter.
- Не зависит от spike spellbook (план 08) — каркас источника-агностичен.

## После реализации
Code-review + арх-ревью через сабагентов. Проверить especially: токенизация кириллицы,
 debounce, тема, что при отсутствии бэкенд-команд нет крашей (catch).
