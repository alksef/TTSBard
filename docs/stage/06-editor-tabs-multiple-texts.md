# Stage: Вкладки для параллельного ввода разных текстов (Editor Tabs)

**Дата:** 2026-06-30
**Статус:** research / ✅ РЕШЕНО (на уровне stage; план будет отдельным файлом)
**Решение:** UX **Вариант A** (табы сверху, 1 активный редактор) + сущность **отдельна от
drafts (plan 75)** + persist **только в памяти сессии** (бэкенд НЕ трогается).
**Из research:** запрос пользователя «меню/табы чтобы параллельно вводить разные тексты»
поверх CodeMirror (план 71).
**Связано:** `01-monaco-vs-codemirror-editor-research.md`, `05-phrase-history.md`,
`docs/plans/71..75`

## Цель (запрос пользователя)
- Иметь возможность **параллельно набирать/держать несколько независимых текстов**,
  переключаясь между ними без потери содержимого.
- Сейчас `text` — единственный `ref('')` в `InputPanel.vue`; переключение панелей (`v-show`
  в `App.vue`) прячет панель ввода целиком, но не даёт «второго текста».
- Нужно UX «как вкладки браузера/IDE»: строка табов над редактором, активный таб → CodeMirror,
  переключение мгновенно.

> **Уточнения, зафиксированные с пользователем (answers):**
> 1. UX = **табы сверху, один активный** (Вариант A). НЕ split-view с несколькими CodeMirror.
> 2. Вкладки — **отдельная сущность от drafts** (plan 75). Drafts = история произнесённого;
>    табы = именованные рабочие черновики. Нет merge, нет дублирования.
> 3. Persist = **только в памяти сессии**. Закрыл окно — табы пропали. Бэкенд НЕ нужен
>    (нет `settings.json`, нет `serde`, нет миграций → нет граблей как с `playback_pause`).

## Контекст кода (что уже есть)
- `InputPanel.vue:13` — `const text = ref('')`. Единственный источник текста.
- `InputPanel.vue:193-200` — `<TtsEditor v-model="text" .../>` монтирует CodeMirror.
- `App.vue:128` — `<InputPanel v-show="currentPanel === 'input'" />`. Один InputPanel,
  переключается между панелями (это панель-навигация, НЕ табы текстов).
- `TtsEditor.vue` (CodeMirror 6):
  - `onMounted` (302-310) создаёт `EditorView`, автофокус.
  - `watch(modelValue)` (317-335) **уже умеет заменять документ** при смене `modelValue`
    через `dispatch({ changes: { from:0, to, insert } })` + флаг `isExternalUpdate`
    (чтобы не зациклить emit). ⇒ При переключении таба CodeMirror сам подхватит новый текст.
  - `defineExpose({ focus })` (341) — можно дёргать фокус при `select`/`create`.
- `useInputHistory.ts` — стиль композабла (ref-состояние, `invoke`, debounce). Образец для
  нового `useEditorTabs.ts`.
- Тема: `TtsEditor.vue` `ttsTheme` использует CSS-vars (`--color-text-primary`,
  `--input-bg-strong`, `--color-border-strong`, `--color-accent`). Любой UI табов обязан
  использовать те же переменные → light/dark работает автоматически (как и везде в приложении).
- Minimal mode: `InputPanel.vue` инжектит `isMinimalMode` (строка 17); `min-height: 340px`
  редактора в minimal становится `280px`. Строка табов должна корректно вести себя в minimal.

---

## Архитектура (решение)

Слой состояния табов — **отдельный Vue-композабл**, фронтенд-only. Бэкенд не трогается.

### Структура данных
```ts
interface EditorTab {
  id: string        // gen id (crypto.randomUUID / счётчик)
  title: string     // пользовательское имя; автогенерация «Текст N»
  text: string      // содержимое вкладки
}
```

### Композабл `src/composables/useEditorTabs.ts` (новый)
```ts
export function useEditorTabs() {
  const tabs = ref<EditorTab[]>([{ id: gen(), title: 'Текст 1', text: '' }])
  const activeId = ref(tabs.value[0].id)

  const active = computed<EditorTab>({
    get: () => tabs.value.find(t => t.id === activeId.value) ?? tabs.value[0],
    set: (v) => { const t = tabs.value.find(t => t.id === activeId.value); if (t) Object.assign(t, v) },
  })

  function create(): string { /* push new tab, select it, focus */ }
  function close(id: string) { /* remove; если активный — выбрать соседа; если последний — создать пустой */ }
  function select(id: string) { /* activeId = id; focus editor */ }
  function rename(id: string, title: string) { /* update title */ }
  // reorder (drag) — опционально, вынести в конец/отдельный этап

  return { tabs, activeId, active, create, close, select, rename }
}
```

### Связка с `InputPanel.vue`
`text` перестаёт быть единственным ref, становится computed-прокси к активному табу:
```ts
const { active } = useEditorTabs()
const text = computed<string>({
  get: () => active.value.text,
  set: (v) => { active.value.text = v },
})
```
`<TtsEditor v-model="text">` **остаётся без изменений** — CodeMirror через `watch(modelValue)`
сам подхватит смену документа при переключении таба (а `update:modelValue` через computed-setter
автоматически пишет обратно в **активный** таб, потери нет).

### UI: новый компонент `src/components/editor/EditorTabs.vue`
- Строка табов над `.textarea-wrapper` в `InputPanel.vue`.
- Каждый таб: `title` + кнопка закрытия `×`. Активный таб подсвечен (`--color-accent`).
- Кнопка `+` — создать таб.
- Дабл-клик по title — переименование (inline `input`).
- Тема — только CSS-vars (light/dark). Minimal-mode: `overflow-x: auto` или сокрытие/сворачивание
  строки табов (см. риски).

### Фокус-менеджмент
- `select(id)` / `create()` → после переключения `nextTick(() => editorRef.value?.focus())`.
  `TtsEditor` уже экспонирует `focus()` (341). Нужна `ref` на `TtsEditor` в `InputPanel`.

---

## Варианты (что рассматривали)

### UX
- **A. Табы сверху, 1 активный** ⭐ выбран. Ложится на существующий InputPanel, CodeMirror не
  пересоздаётся. Знакомый паттерн.
- **B. Split-view, несколько CodeMirror одновременно.** Отвергнут: CodeMirror 6 тяжёлый
  (3-5 инстансов = память), layout/высоты ломаются (жёсткий `min-height: 340px`), фокус сложнее.
- **C. Боковое меню черновиков.** Отвергнут как отдельная сущность — сливается с plan 75 drafts.

### Persist
- **Только в памяти сессии** ⭐ выбран (по ответу пользователя). Фронтенд-only, нулевой риск
  миграции, самая быстрая реализация.
- Сохранение на диск — отвергнуто: потребовало бы бэкенд (`settings.json`/`tabs.json` + `serde`),
  а это те же грабли миграции, что с `playback_pause` (без `#[serde(default)]` ломает запуск на
  старых конфигах). Не оправдано для «рабочих черновиков».

### Отношение к drafts (plan 75)
- **Отдельная сущность** ⭐ выбран. Табы = активные именованные рабочие тексты (in-memory);
  drafts = журнал произнесённого (persistent, отдельный источник правды). Нет дублирования, нет
  конфликта моделей.

---

## Риски / нюансы (для будущего плана)
1. **Сброс курсора/скролла при переключении таба** — `TtsEditor.watch` заменяет весь документ;
   курсор уходит в начало. Нормально для табов, но зафиксировать как ожидаемое поведение.
2. **Minimal-mode** — узкое окно: строка табов может не влезть. Заложить `overflow-x: auto`
   и/или скрытие табов в minimal (как `correct-button` скрыт через `v-if="!isMinimalMode"`).
3. **Закрытие последнего таба** — не оставлять пустое состояние: всегда держать ≥1 таб
   (close последнего = создать пустой и выбрать его).
4. **Закрытие таба с несохранённым текстом** — подтвердить/не подтверждать? Решение для stage:
   БЕЗ подтверждения (in-memory, текст не критичен; соответствует «только в памяти сессии»).
5. **Коллизия с quick-editor mode** (`editorSettings.quick`) — `handleEnter` (InputPanel.vue:143)
   очищает `text.value = ''` после speak. С computed-прокси это очистит **активный** таб —
   корректно, но зафиксировать в плане.
6. **id-генерация без `Date.now()`/`Math.random()` в sandbox-контекстах** — во фронте можно
   `crypto.randomUUID()`; если планируется workflow-тест, предпочесть счётчик. Мелочь.

---

## Оценка трудозатрат
Фронтенд-only, небольшая задача:

| Часть | Объём |
|---|---|
| `useEditorTabs.ts` (composable, CRUD) | малый |
| `EditorTabs.vue` + тема (light/dark, minimal) | малый-средний |
| Связка с `InputPanel` (computed-прокси `text`, ref на editor для focus) | малий |
| Бэкенд | **нет** |

Уровень ~ plan 72, заметно меньше plan 74. Реализация — через DeepSeek (CLAUDE.md workflow);
Claude пишет только план/ревью.

## KEY_DECISIONS
- **Табы сверху (A)**, один активный CodeMirror — не split-view.
- **Отдельно от drafts** (plan 75) — две сущности, без merge.
- **Только в памяти сессии** — бэкенд не трогается, нулевой риск миграции настроек.
- **Тема через CSS-vars** — light/dark автоматически, как в `TtsEditor`.
- **Computed-прокси `text`** в InputPanel — `TtsEditor` остаётся неизменным.
