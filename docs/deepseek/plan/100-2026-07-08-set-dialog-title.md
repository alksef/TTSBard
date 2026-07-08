# План 100: Bug — заголовок диалога создания набора звуков «Сообщение с tauri.localhost»

- **Дата:** 2026-07-08
- **Тип:** bug / UX (frontend, SoundPanelTab.vue)
- **Симптом (от пользователя):** «при добавлении нового набора звуков заголовок вложенного окна
  "Сообщение с tauri.localhost" — переименовать»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Корень проблемы (найден Claude)

`src/components/SoundPanelTab.vue:71-82` — функция `addSet()`:
```ts
async function addSet() {
  const name = prompt('Имя набора:')          // ← window.prompt() — нативный браузерный диалог
  if (!name || !name.trim()) return
  try {
    const created = await invoke<SoundSet>('sp_add_set', { name: name.trim() })
    ...
```

Используется **`window.prompt()`** — системный диалог браузера/WebView. В Tauri его заголовок
неконтролируемый («Сообщение с tauri.localhost» — это дефолтный заголовок WebView2 для `prompt`).
Его нельзя стилизовать.

В этом же файле уже используется **`@tauri-apps/plugin-dialog`** — `confirm` (line 117, 169) с
настраиваемым `title`. Нужно `prompt`-аналог: либо нативный диалог из плагина, либо свой
модальный компонент.

## Решение (варианты — выбрать в задаче)

### Вариант A — Свой модальный диалог (рекомендуется, консистентно с UI)
В файле уже есть модальный диалог добавления звука (`.dialog-overlay` / `.dialog`,
`SoundPanelTab.vue:491-559`). Сделать аналогичный маленький модал для ввода имени набора:
- `showAddSetDialog` ref, `newSetName` ref.
- Заголовок «Новый набор звуков», input, кнопки Отмена/Создать.
- Плюс: единый стиль с темой (CSS vars), нет нативного окна, фокус на input.
- Минус: ещё один модал (но маленький).

### Вариант B — `@tauri-apps/plugin-dialog` нативный prompt
Плагин `dialog` экспортирует `ask`/`confirm`/`message`, но **не `prompt`** (в Tauri v2 plugin-dialog
нет встроенного text-input диалога). Проверить версию плагина — если нет prompt, вариант B отпадает.

**Рекомендация: Вариант A** — свой модал. Стиль уже есть в файле, переиспользовать.

---

## Задача DeepSeek

### Реализация (Вариант A)
1. Удалить `window.prompt()` из `addSet()`.
2. Добавить state: `showAddSetDialog = ref(false)`, `newSetName = ref('')`.
3. `addSet()` → открывает модал (`showAddSetDialog.value = true`, очистка input, фокус).
4. Новый `confirmAddSet()` — валидация (не пусто, trim), `invoke('sp_add_set', { name })`,
   закрытие. Обработка ошибки как сейчас (`showError`).
5. Шаблон модала — по образцу существующего `.dialog-overlay` (line 491), но проще: заголовок
   «Новый набор звуков», один input, кнопки. Переиспользовать существующие CSS-классы
   (`.dialog`, `.text-input`, `.dialog-actions`, `.cancel-button`, `.save-button`).
6. Поддержка Enter (подтвердить) и Escape (отмена) в input — `@keydown.enter`, `@keydown.esc`.
7. maxlength=50 (как у rename input).

### Edge cases
- Пустое имя / только пробелы — кнопка «Создать» disabled или валидация в `confirmAddSet`.
- Дубликат имени набора — backend не запрещает (можно), не блокировать. (Опционально: warning.)
- Модал должен закрываться по клику на overlay (как существующий `@click="closeAddDialog"`).

---

## Верификация
- `vue-tsc --noEmit` 0 ошибок.
- **Runtime:** нажать «+» (новый набор) → открывается **свой** модал с заголовком «Новый набор
  звуков» (НЕ «Сообщение с tauri.localhost»). Ввести имя → набор создаётся. Escape — отмена.
  Enter — создать.

## Не делать
- Не трогать rename (там inline-input в табе, это другое).
- Не менять backend (`sp_add_set`).
