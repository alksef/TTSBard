# Plan 112: Аудио-команды — ephemeral SettingsManager → managed State

**Дата:** 2026-07-11  
**Источник:** review-001-2026-07-11 (MINOR, split-brain кешей)  
**Сложность:** Низкая — механическая миграция 8 команд.

---

## Проблема

8 аудио-команд в `src-tauri/src/commands/mod.rs` (строки 851–912) создают
ephemeral `SettingsManager::new()` вместо использования managed State:

```rust
// Текущий паттерн (НЕПРАВИЛЬНО):
pub fn get_audio_settings() -> Result<AudioSettings, String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.load())
        .map(|s| s.audio)
        .map_err(|e| e.to_string())
}
```

Проблема: каждый ephemeral-экземпляр имеет **собственный `Arc<RwLock<AppSettings>>`** —
кеш в памяти. Обновления через managed-экземпляр (который используют ~40 других команд)
не видны ephemeral-экземплярам и наоборот. Split-brain.

---

## Решение

Мигрировать все 8 команд на паттерн `State<'_, SettingsManager>` — как это уже
сделано для других команд (например, строка 290, 379, 445 и т.д.).

### Список команд для миграции

| Строка | Команда |
|--------|---------|
| 851 | `get_audio_settings` |
| 860 | `set_speaker_device` |
| 868 | `set_speaker_enabled` |
| 876 | `set_speaker_volume` |
| 884 | `set_virtual_mic_device` |
| 892 | `enable_virtual_mic` |
| 900 | `disable_virtual_mic` |
| 908 | `set_virtual_mic_volume` |

### Паттерн миграции

```rust
// БЫЛО:
#[tauri::command]
pub fn set_speaker_device(device_id: Option<String>) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_device(device_id))
        .map_err(|e| e.to_string())
}

// СТАЛО:
#[tauri::command]
pub fn set_speaker_device(
    device_id: Option<String>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager
        .set_speaker_device(device_id)
        .map_err(|e| e.to_string())
}
```

---

## Проверка
- `cargo check` — 0/0.
- `SettingsManager::new()` не должен вызываться в этих 8 командах после фикса.
- Сигнатуры в `lib.rs` (invoke_handler) не требуют изменений — Tauri автоматически
  инжектирует State по типу.
