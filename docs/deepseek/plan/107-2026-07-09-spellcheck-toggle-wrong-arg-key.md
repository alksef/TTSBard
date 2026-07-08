# План 107: Bug — переключение орфографии падает: «missing required key value»

- **Дата:** 2026-07-09
- **Тип:** bug (frontend ↔ backend command arg mismatch)
- **Симптом (от пользователя):** включение проверки орфографии (offline) даёт ошибку:
  `Ошибка переключения орфографии: invalid args \`value\` for command
  \`set_editor_spellcheck_enabled\`: command set_editor_spellcheck_enabled missing required key value`
  (URL: `http://ipc.localhost/set_editor_spellcheck_enabled`)
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Корневая причина

Несоответствие имени аргумента между фронтом и бэкендом.

**Бэкенд** (`src-tauri/src/commands/mod.rs:1059`):
```rust
#[tauri::command]
pub fn set_editor_spellcheck_enabled(
    value: bool,                       // ← параметр называется `value`
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<bool, String> { ... }
```
Tauri маппит имя аргумента команды → ключ в JSON-пэйлоаде invoke. Для параметра `value`
бэкенд ждёт ключ `value` (Tauri 2 оставляет однобуквенные/короткие snake-имена как есть, не
делая camelCase).

**Фронт** (`src/components/settings/SettingsEditor.vue:18-21`):
```ts
async function toggleSpellcheck() {
  ...
  await invoke('set_editor_spellcheck_enabled', { enabled: newValue })  // ← шлёт `enabled`
  ...
}
```
Фронт шлёт ключ `enabled`, а бэкенд ждёт `value` → `missing required key value`.

### Доказательство (тот же файл, рабочий аналог)

В `SettingsEditor.vue:32` соседний тоггл зовёт **правильно**:
```ts
await invoke('set_editor_quick', { value: newValue });   // ← `value` ✅
```
а `set_editor_spellcheck_enabled` — с `enabled` (опечатка/несоответствие). Одна и та же
команда-паттерн, разные имена ключа.

---

## Фикс

`src/components/settings/SettingsEditor.vue:21`:
```ts
// было:
await invoke('set_editor_spellcheck_enabled', { enabled: newValue })
// стало:
await invoke('set_editor_spellcheck_enabled', { value: newValue })
```

Имя ключа привести в соответствие с параметром команды (`value`). Это точечная правка одной
строки.

### Проверить родственные команды (на всякий случай)

В том же файле/компоненте поискать другие `invoke(...)` к spellcheck-командам и сверить имена
ключей с параметрами бэкенда:
- `set_editor_spellcheck_source` (`src-tauri/src/commands/mod.rs:1082`, параметр `value:
  SpellSource`) — если фронт зовёт её, ключ должен быть `value`, не `source`.
- `get_editor_spellcheck_enabled` / `get_editor_spellcheck_source` — без аргументов, не affected.

Если где-то ещё рассинхрон — поправить тем же образом (сверить с сигнатурой `#[tauri::command]`).

---

## Верификация

1. `npx vue-tsc --noEmit` — 0/0.
2. **Runtime:**
   - Настройки редактора → включить проверку орфографии → **ошибка больше не появляется**,
     показывается «Настройка сохранена».
   - Слова с ошибками подчёркиваются (offline-режим — см. `docs/bugs/02-...` про онлайн-режим,
     это отдельный баг, тут не трогать).
   - Выключить/включить несколько раз — стабильно сохраняется (проверить `settings.json`:
     `spellcheck_enabled` меняется).

## Не делать
- Не трогать онлайн-режим орфографии (`check_spelling_online`) — это баг BUG-02
  (`docs/bugs/02-spellcheck-online-source-calls-missing-command.md`), отдельная задача.
- Не переименовывать параметр на бэкенде (меньше изменений) — править фронт под существующий
  контракт `value`. Если по каким-то причинам `value` неудобно семантически — обсудить, но
  дефолт — фронт.
