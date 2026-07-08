# План 97: Bug — клавиши A-Z саундпанели завязаны на раскладку клавиатуры

- **Дата:** 2026-07-08
- **Тип:** bug (frontend, плавающее окно саундпанели)
- **Симптом (от пользователя):** «в звуковой панели символы не должны быть завязаны на раскладку»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Корень проблемы (найден Claude)

Плавающее окно саундпанели обрабатывает нажатия A-Z через DOM `keydown` (не через low-level hook —
см. комментарий в `src-tauri/src/soundpanel/hook.rs:5`: «A-Z/Escape обрабатываются самим окном через
DOM keydown»).

`src-soundpanel/SoundPanelApp.vue:141-159`:
```ts
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') { closeWindow(); return }
  if (e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) return
  const key = e.key.toUpperCase()          // ← ЗАВИСИТ ОТ РАСКЛАДКИ
  if (!/^[A-Z]$/.test(key)) return
  const b = bindings.value.find(x => x.key === key)
  ...
  invoke('sp_play_binding', { key })
}
```

**`e.key` даёт символ по текущей раскладке.** На русской раскладке физическая клавиша **A**
даёт `e.key === "ф"` → `"Ф"` → не проходит `/^[A-Z]$/` → звук не играет. То же для всех букв:
привязка на «A» не сработает, пока активна RU-раскладка.

### Правильное решение: `e.code` (layout-независимо)
`KeyboardEvent.code` — физическая позиция клавиши, не зависит от раскладки/языка:
- Физическая A → `e.code === "KeyA"` всегда (любая раскладка).
- Диапазон: `"KeyA"` … `"KeyZ"`.

Нужно извлекать букву из `e.code`, а не из `e.key`.

---

## Задача DeepSeek

### Фикс `SoundPanelApp.vue` `onKeydown`
Заменить логику извлечения клавиши на `e.code`-основанную:

```ts
function codeToLetter(code: string): string | null {
  // "KeyA".."KeyZ" → "A".."Z"
  if (code.length === 4 && code.startsWith('Key')) {
    const letter = code[3].toUpperCase()
    if (letter >= 'A' && letter <= 'Z') return letter
  }
  return null
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') { closeWindow(); return }
  if (e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) return
  const key = codeToLetter(e.code)
  if (!key) return
  const b = bindings.value.find(x => x.key === key)
  if (b) {
    e.preventDefault()
    invoke('sp_play_binding', { key }).then(() => closeWindow())
  } else {
    showNoBinding(key)
  }
}
```

### Чек-лист edge-cases
- `e.code` для букв A-Z — всегда `"KeyA".."KeyZ"` независимо от раскладки, CapsLock, Shift.
  Shift уже отсеивается выше (модификаторы return), но даже с Shift code не меняется — безопасно.
- Не сломать Escape (оставить `e.key === 'Escape'` — Escape это всегда `Escape` в `e.key`).
- `showNoBinding(key)` теперь получает корректную A-Z даже на RU-раскладке (сообщение «нет
  привязки для A» будет правильным).
- Backend `sp_play_binding` (bindings.rs:186) уже валидирует `key_char.is_ascii_uppercase()` —
  принимает A-Z, никаких изменений бэкенда не нужно.

### Не трогать
- `hook.rs` — NumPad/F-keys через `vk_to_name(vk_code)` уже layout-независимы (виртуальные
  коды). Только A-Z были на DOM `e.key`.
- `SoundPanelTab.vue` — это конфиг-UI, не играет звуки по клавишам. Его не трогать.

---

## Верификация
- `vue-tsc --noEmit` 0 ошибок.
- **Runtime (обязательно):** переключить раскладку на RU → открыть саундпанель (`Ctrl+Shift+F2`)
  → нажать физическую клавишу с привязкой (напр. A) → звук играет. На EN — тоже играет. Раньше
  на RU не работало.
- Edge: CapsLock включён — поведение не меняется (code не зависит).

## Примечание
Это **чисто фронтенд** правка одного файла (`SoundPanelApp.vue`). Маленький, изолированный фикс.
