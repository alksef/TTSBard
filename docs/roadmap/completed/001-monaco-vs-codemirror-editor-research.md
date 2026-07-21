# ROADMAP-001 — Переход на продвинутый редактор

**Дата:** 2026-06-28
**Статус:** research / ✅ РЕШЕНО
**Решение:** **CodeMirror 6** (подтверждено пользователем)
**Связано:** `02-local-history-autocomplete.md`, `03-text-completion-without-ai.md`

---

## Контекст

Сейчас ввод текста в приложении — обычный HTML `<textarea>` (в WebView-шаблонах даже с
`spellcheck="false"`, см. `docs/plans/22-2026-03-06-webview-source.md`).

Пользователь хочет заменить `<textarea>` на «продвинутый редактор», и читал, что **Monaco**
ближе всех к VSCode (потому что Monaco — это и есть движок VSCode). Цель этого файла —
проверить, оптимален ли Monaco для нашего случая, и зафиксировать обоснованный выбор.

> **Важно:** этот файл — исследование, а не план. Реализация пойдёт отдельным планом в
> `docs/deepseek/plan/`, код пишет DeepSeek (см. `CLAUDE.md` → Implementation Workflow).

---

## TL;DR

**Monaco «ближе всех к VSCode» — правда, но для нашего кейса он НЕ оптимален.**
Рекомендованный вариант — **CodeMirror 6**. Финальное решение — за пользователем.

---

## Сравнение Monaco vs CodeMirror 6

| Критерий | Monaco Editor | CodeMirror 6 | Победитель |
|---|---|---|---|
| Происхождение | Движок VSCode (Microsoft) |独立的, современная архитектура | Monaco «по фам» |
| **Размер бандла** | ~5 МБ gzip, тяжело lazy-load | ~50–200 КБ, tree-shakeable, модульный | **CM6** |
| Интеграция с **Vue 3 + Vite** | Требует спец. ESM/loader-конфиг (`@guolao/vue-monaco-editor`), частые проблемы со сборкой | Из коробки работает с Vue 3 + Vite | **CM6** |
| Интеграция с **Tauri** | Работает, но тяжелее по памяти/бандлу | Работает, нужен только CSP `style-src 'unsafe-inline'` (inline-стили) | **CM6** |
| Autocomplete | Встроенный IntelliSense (заточен под код) | Через расширение `@codemirror/autocomplete` (затачивается под наш кейс) | Ничья (CM6 гибче) |
| Multiline / обычный текст | Есть, но по умолчанию «IDE-look» (minimap и т.п.) | Полностью, легко сделать минималистично под обычный текст | **CM6** |
| Однострочное/минимальное поле | Сложно настроить под «не-код» | Легко | **CM6** |
| Производительность на больших текстах | Тянет 100k+ строк, но тяжелее | Ленивый рендер, эффективен | **CM6** |
| Кастомизация | Даёт «целую IDE» по умолчанию, менее гибко | Высоко модульный — берём только нужное | **CM6** |
| Поддержка spellcheck / линтеров | Возможна, но тяжеловесно | Лёгкие lint-расширения (хорошо ложится на hunspell) | **CM6** |
| Мобильная отзывчивость | Desktop-first | Заточен и под легковесные/мобильные сценарии | **CM6** |

### Почему Monaco избыточен для нас
1. **5 МБ в бандле Tauri-приложения** — ощутимо, особенно при inline-встраивании.
2. **Заточен под редактирование кода**, а не обычного пользовательского текста — много
   ненужного (minimap, IntelliSense по языкам, модель «файл с ошибками»).
3. **Проблемы со сборкой в Vue/Vite** — Monaco рассчитан на загрузчик AMD, в ESM-сборке
   Vue нужны обёртки (`@guolao/vue-monaco-editor`), что добавляет хрупкости.
4. **CSP/inline-стили в Tauri** — Monaco агрессивно инжектит стили; CM6 тоже требует
   `style-src 'unsafe-inline'`, но это единственное ограничение.

### Почему CodeMirror 6 лучше подходит
- Модульность: для обычного текста берём `EditorView`, `@codemirror/autocomplete`,
  (опц.) `@codemirror/language` + своё расширение для русского словаря.
- Меньше бандл → быстрее старт приложения и меньше вес инсталлера.
- Та же инфраструктура `autocomplete`-расширений даёт и автокомплит слов из истории,
  и слоты для spellcheck-линтера (см. связные файлы).
- Лучше совместимость с Vue 3 + Vite + Tauri (минимум подводных камней).

---

## Когда всё-таки стоит выбрать Monaco

- Если в будущем планируется **полноценный редактор скриптов/шаблонов с подсветкой синтаксиса
  нескольких языков, множественными курсорами, diff-режимом** и «ощущением VSCode».
- Если размер бандла (5 МБ) для проекта приемлем.

В текущем скоупе (обычный пользовательский текст для TTS) это не требуется.

---

## Открытые вопросы для пользователя
1. ~~Окончательный выбор~~ → **CodeMirror 6** (решено).
2. Сохранить поведение «быстрого редактора» (Enter/Esc) — **да**, через keymap расширение CM6
   (логика `handleEnter/Esc` из `InputPanel.vue` переносится в keymap).
3. Поддержка тем: новый редактор следует единой системе тем (`data-theme` + CSS-переменные),
   CM6-тема строится поверх `src/styles/variables.css`.

## Точки интеграции (по результатам исследования кода)
- **Frontend:** `src/components/InputPanel.vue` — `const text = ref('')` (строка 12),
  `<textarea v-model="text">` (строки 222-232), `handleEnter` (133-154),
  `handleEsc` (156-164), `handleSpace` (166-215). Текст сейчас не персистится.
- **Стек:** Vue 3.5.30 (Composition API, `<script setup lang="ts">`), TypeScript 6,
  Vite 8, npm (есть `package-lock.json`). **Pinia нет** — composables (`useAppSettings.ts`).
- **Темы:** `src/styles/variables.css` (CSS-переменные, `--font-mono: 'JetBrains Mono'`),
  переключение через `data-theme` в `App.vue`.

---

## Источники
- [Monaco vs CodeMirror 6 — сравнение 2026](https://www.pkgpulse.com/guides/monaco-editor-vs-codemirror-6-vs-sandpack-in-browser-2026)
- [CodeMirror vs Monaco Editor comparison](https://agenthicks.com/research/codemirror-vs-monaco-editor-comparison)
- [Sourcegraph: миграция Monaco → CodeMirror](https://sourcegraph.com/blog/migrating-monaco-codemirror)
- [CodeMirror + Tauri/SvelteKit/Vite: CSP и prod-сборка](https://discuss.codemirror.net/t/tauri-sveltekit-vite-codemirror-6-works-in-dev-breaks-in-production-build/9339)
- [Интеграция Monaco Editor (обёртки, нюансы)](https://www.spectralcore.com/blog/integrating-monaco-editor)
- [@guolao/vue-monaco-editor](https://www.npmjs.com/package/@guolao/vue-monaco-editor)
- [Ace / CodeMirror / Monaco — сравнение](https://www.reddit.com/r/javascript/comments/s1e55h/ace_codemirror_and_monaco_a_comparison_of_the/)
