# Task 117-phase-A-01: Разделение commands/mod.rs на существующие подмодули

План: `docs/plans/117-2026-07-11-appstate-decomposition-and-commands-refactoring.md` (читать обязательно).

## Описание задачи
Файл `src-tauri/src/commands/mod.rs` содержит около 1400 строк. Наша цель — разгрузить его, перенеся группы связанных Tauri-команд в уже существующие файлы-подмодули в директории `src-tauri/src/commands/`.

Чтобы не ломать сборку и импорты в `lib.rs`, мы будем использовать механизм ре-экспорта: в `commands/mod.rs` после переноса функций мы добавим `pub use self::ai::*;`, `pub use self::playback::*;`, `pub use self::window::*;` и т.д. Таким образом, внешний API модуля `commands` останется абсолютно идентичным!

---

## 1. Группа 1: Аудио и Воспроизведение -> `commands/playback.rs`

Перенеси следующие функции из `commands/mod.rs` в конец `commands/playback.rs`:
- `get_output_devices`
- `get_virtual_mic_devices`
- `get_audio_settings`
- `set_speaker_device`
- `set_speaker_enabled`
- `set_speaker_volume`
- `set_virtual_mic_device`
- `enable_virtual_mic`
- `disable_virtual_mic`
- `set_virtual_mic_volume`
- `test_audio_device`
- `get_audio_effects`
- `set_audio_effects_enabled`
- `set_audio_effects_pitch`
- `set_audio_effects_speed`
- `set_audio_effects_volume`

Убедись, что в `commands/playback.rs` импортированы все нужные зависимости (например, `State`, `SettingsManager`, `AppState`, `tracing::*`, `OutputDeviceInfo` и т.д.).

---

## 2. Группа 2: Настройки AI и TTS провайдеров -> `commands/ai.rs`

Перенеси следующие функции из `commands/mod.rs` в конец `commands/ai.rs`:
- `get_tts_provider`
- `set_tts_provider`
- `get_local_tts_url`
- `set_local_tts_url`
- `get_openai_api_key`
- `set_openai_api_key`
- `get_openai_voice`
- `set_openai_voice`
- `apply_openai_proxy_settings`
- `get_fish_audio_api_key`
- `set_fish_audio_api_key`
- `get_fish_audio_reference_id`
- `set_fish_audio_reference_id`
- `get_fish_audio_voices`
- `add_fish_audio_voice`
- `remove_fish_audio_voice`
- `set_fish_audio_format`
- `set_fish_audio_temperature`
- `set_fish_audio_sample_rate`
- `set_fish_audio_use_proxy`
- `apply_fish_audio_proxy_settings`
- `has_api_key`

Убедись, что в `commands/ai.rs` импортированы необходимые типы (например, `TtsProviderType`, `TelegramState` и т.д.).

---

## 3. Группа 3: Окна, Хоткеи и Системные команды -> `commands/window.rs`

Перенеси следующие функции из `commands/mod.rs` в конец `commands/window.rs`:
- `get_interception`
- `set_interception`
- `toggle_interception`
- `get_hotkey_enabled`
- `set_hotkey_enabled`
- `get_global_exclude_from_capture`
- `set_global_exclude_from_capture`
- `update_theme`
- `hide_main_window`
- `close_soundpanel_window`
- `toggle_playback_control_window`
- `set_show_playback_on_start`
- `get_show_playback_on_start`
- `get_hotkey_settings`
- `set_hotkey`
- `reset_hotkey_to_default`
- `unregister_hotkeys`
- `reregister_hotkeys_cmd`
- `set_hotkey_recording`
- `open_file_dialog`

---

## 4. Обновление `src-tauri/src/commands/mod.rs`

После переноса функций удали их из `commands/mod.rs`.
В `commands/mod.rs` должны остаться:
1. Декларации модулей (`pub mod ai;`, `pub mod playback;`, `pub mod window;` и т.д.)
2. Ре-экспорты всех функций из подмодулей, чтобы они были доступны через `commands::*`:
   ```rust
   pub use self::ai::*;
   pub use self::playback::*;
   pub use self::window::*;
   // ... другие ре-экспорты если нужны ...
   ```
3. Функции:
   - `quit_app`
   - `speak_text`
   - `speak_text_internal`

---

## Верификация
1. `npx vue-tsc --noEmit` — 0 ошибок.
2. `cargo check` — 0 ошибок (убедись, что все импорты в файлах на месте).
3. В отчёте: покажи структуру файлов в `commands/` после завершения переноса и подтверди отсутствие ошибок.
