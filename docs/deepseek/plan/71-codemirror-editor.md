# DeepSeek Plan 71: Переход на продвинутый редактор (CodeMirror 6)

> **Для DeepSeek:** пиши реализацию сам. Здесь — инструкции (что делать, какие файлы/типы/
> сигнатуры/поведение), а не готовый код. Общий план и обоснование — `docs/plans/71-...`.
> Контекст кода — `docs/stage/01-...`.

## Контекст проекта
- Стек: Vue 3.5.30 (`<script setup lang="ts">`), TypeScript 6, Vite 8, npm, Tauri 2.x.
- Pinia НЕТ — composables (`src/composables/useAppSettings.ts`).
- Текущий ввод: `src/components/InputPanel.vue`, `const text = ref('')` (строка 12),
  `<textarea v-model="text">` (строки 222-232).
- Поведение: `handleEnter` (133-154, quick-editor через `editor.quick`), `handleEsc` (156-164),
  `handleSpace` (166-215, автозамена `\word` / `%username`).
- Темы: `src/styles/variables.css`, переключение `data-theme` в `App.vue` (`general.theme`).
- Шрифт: `--font-mono: 'JetBrains Mono'`.

## Что сделать

### 1. Зависимости
- Добавь в `package.json` пакеты CodeMirror 6: `@codemirror/view`, `@codemirror/state`,
  `@codemirror/commands`, `@codemirror/language`, `@codemirror/autocomplete`, `@codemirror/search`.
- Установи через npm.

### 2. Компонент-обёртка `src/components/editor/TtsEditor.vue`
- Vue 3 SFC, `<script setup lang="ts">`.
- API совместимо с v-model: проп `modelValue: string`, эмит `update:modelValue`.
- Монтирует CM6 в `<div ref>` через `EditorState.create` + `EditorView`.
- Синхронизация: изменение `modelValue` снаружи → обновляет документ (без зацикливания;
  сравнивай с текущим документом); ввод пользователем → эмит `update:modelValue`.
- Проп `placeholder: string`, автофокус при монтировании.
- Внутри используй расширения CM6: базовый `keymap.of([...defaultKeymap, ...historyKeymap])`,
  `EditorView.lineWrapping` (перенос строк), тема (п.3), keymap проекта (п.4).

### 3. Тема
- Создай CM6-тему (через `EditorView.theme`), читающую значения из CSS-переменных
  `src/styles/variables.css` (фон, текст, курсор, выделение, семейство шрифта `--font-mono`).
- Тема должна реагировать на `data-theme` (light/dark) — пробрасывай CSS-переменные,
  не хардкодь цвета.

### 4. Keymap (сохранить поведение быстрого редактора)
- Реализуй keymap-расширение CM6, переносящее логику из `InputPanel.vue`:
  - **Enter** (без Shift): если `editor.quick` включён и текст не пустой → speak + очистить +
    скрыть главное окно; иначе speak + очистить. (Это уже вызывает backend — переиспользуй
    существующую логику `handleEnter`, не дублируй TTS-вызовы.)
  - **Esc**: в quick-режиме — скрыть окно.
  - **Space**: автозамена `\replacement` / `%username` (логика `handleSpace`).
- Эти обработчики должны получать доступ к current document и триггерить существующие
  функции (speak/hideMainWindow) — вынеси их в composable или props/emits, чтобы не
  дублировать TTS-логику внутри компонента редактора.

### 5. CSP для prod-сборки Tauri
- Проверь/обнови CSP в Tauri-конфиге так, чтобы разрешить inline-стили
  (`style-src` должен допускать `'unsafe-inline'` или nonce, который использует CM6).
  Это требуется для корректного рендера CM6 в production.

### 6. Интеграция в `InputPanel.vue`
- Замени `<textarea>` (строки 222-232) на `<TtsEditor v-model="text" placeholder="..." />`.
- Убедись, что `text`, `handleEnter/Esc/Space`, quick-режим остаются рабочими через новую
  обёртку (логику можно поднять в `InputPanel` и передавать в `TtsEditor` через
  props/emits, либо через composable).

## Ограничения / требования
- Не добавляй автокомплит — он в плане 72 (только подготовь место: расширения подключаются
  через `EditorState.create({ extensions: [...] })`, чтобы потом добавить autocomplete-источник).
- Не хардкодь цвета — только через CSS-переменные.
- TypeScript строгий — без `any`.
- Не ломай существующие hotkeys/панели.
- После реализации: `npm run build` (`vue-tsc --noEmit && vite build`) и `cargo check` в
  `src-tauri/` должны проходить.

## Критерии готовности
- Ввод/редактирование работает.
- Quick-editor (Enter/Esc), space-автозамена работают как раньше.
- Светлая/тёмная тема корректна.
- Prod-сборка Tauri рендерит редактор (CSP проверен).

---
**Статус: ВЫПОЛНЕНО** (28.06.2026)
- Установлены CM6 зависимости (@codemirror/view, state, commands, language, autocomplete, search)
- Создан `src/components/editor/TtsEditor.vue` — обёртка CM6 с v-model, placeholder, автофокусом
- Реализована CM6-тема через CSS-переменные (фон, текст, курсор, выделение, --font-mono)
- Keymap: Enter → speak+очистить, Esc → скрыть окно, Space → автозамена \word / %username
- CSP в tauri.conf.json = null (нет ограничений)
- `<textarea>` в InputPanel.vue заменён на `<TtsEditor>` с v-model и событиями enter/esc
- Все хоткеи/панели сохранены
- `vue-tsc --noEmit` и `vite build` проходят; `cargo check` проходит
