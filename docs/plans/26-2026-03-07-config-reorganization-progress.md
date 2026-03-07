# Config Reorganization - Progress Report

**Date:** 2026-03-07
**Status:** Backend Complete ✅

## Completed Tasks

### 1. Created config module structure ✅
- `src-tauri/src/config/mod.rs`
- `src-tauri/src/config/validation.rs`
- `src-tauri/src/config/windows.rs`
- `src-tauri/src/config/settings.rs`

### 2. Updated core files ✅
- Updated `state.rs` - removed window/audio fields, kept only runtime state, added hotkey_enabled runtime field
- Updated `lib.rs` - added config module, updated startup code, sync hotkey_enabled from settings
- Updated `audio/mod.rs` - removed settings export
- Deleted `audio/settings.rs`

### 3. Updated imports ✅
- `commands/mod.rs` - using `config::{SettingsManager, WindowsManager}`
- `commands/webview.rs` - using `config::SettingsManager`
- `commands/twitch.rs` - using `config::TwitchSettings`
- `commands/telegram.rs` - using `config::SettingsManager`
- `floating.rs` - using `config::WindowsManager`
- `state.rs` - using `config::TwitchSettings`
- `soundpanel/bindings.rs` - using `config::WindowsManager`

### 4. Updated key functions ✅
- `quit_app` - using `WindowsManager`
- `toggle_floating_window` - using `WindowsManager`
- `show_floating_window_cmd` - using `WindowsManager`
- `hide_floating_window_cmd` - using `WindowsManager`
- `set_local_tts_url` - using `SettingsManager`
- `get_local_tts_url` - using `SettingsManager`
- Audio commands - using `SettingsManager`
- `get/set_hotkey_enabled` - using both `SettingsManager` and `AppState`
- SoundPanel appearance commands - using `WindowsManager`

### 5. Fixed compilation errors ✅
- Added `has_api_key` to imports in lib.rs
- Added `is_hotkey_enabled()` and `set_hotkey_enabled()` to AppState
- Added `hotkey_enabled` runtime field to AppState
- Sync `hotkey_enabled` from SettingsManager to AppState in setup
- Fixed `TwitchSettings` name conflict with `From` trait (ConfigTwitchSettings alias)
- Code compiles successfully!

### 6. Updated Telegram config ✅
- Removed `telegram_config.json` handling from `telegram/client.rs`
- Updated `commands/telegram.rs` to use `SettingsManager`:
  - `telegram_sign_in` now saves api_id to settings.json
  - `telegram_sign_out` now deletes api_id from settings.json
  - `telegram_auto_restore` now loads api_id from settings.json
- Removed unused functions: `save_api_id`, `load_api_id`, `delete_config`, `TelegramConfig`

### 7. Migrated SoundPanel appearance ✅
- Updated `soundpanel/storage.rs`:
  - `load_appearance()` now reads from `WindowsManager` instead of `soundpanel_appearance.json`
  - Removed `save_appearance()` and `save_appearance_direct()` functions
  - Added comment explaining migration
- Updated `soundpanel/bindings.rs`:
  - All appearance commands now use `WindowsManager` for saving
  - `sp_set_floating_opacity`, `sp_set_floating_bg_color`, `sp_set_floating_clickthrough`, `sp_set_exclude_from_recording`
- Updated `lib.rs` to pass `WindowsManager` to `load_appearance()`
- Removed import of `save_appearance` from bindings.rs

## File Structure (Final)

```
%APPDATA%\ttsbard\
├── windows.json              # All window settings ✅
├── settings.json             # Main app settings ✅
├── soundpanel_bindings.json  # (unchanged) ✅
├── replacements.txt          # (unchanged) ✅
├── usernames.txt             # (unchanged) ✅
├── telegram.session          # (unchanged) ✅
├── soundpanel/               # (unchanged) ✅
└── webview/
    ├── template.html         # (unchanged) ✅
    └── style.css             # (unchanged) ✅
```

**Deleted Files:**
- `audio_settings.json` → now in `settings.audio`
- `twitch/settings.json` → now in `settings.twitch`
- `webview/settings.json` → now in `settings.webview`
- `telegram_config.json` → now in `settings.tts.telegram.api_id`
- `soundpanel_appearance.json` → now in `windows.soundpanel`
- `settings.json` (old) → split into `settings` + `windows`

## Remaining Work

### Cleanup (optional)
- Clean up unused warnings (dead_code)
- Remove unused imports

## Current Status

✅ Backend complete and compiles successfully
✅ Config files auto-created on first run with correct defaults
✅ settings.json structure verified (audio, tts, twitch, webview)
✅ windows.json structure verified (main, floating, soundpanel)
✅ Application starts successfully with new config format
✅ Old settings backed up to bak/old-config/

## Implementation Complete! 🎉

The config reorganization has been successfully implemented:

**New Config Files:**
- `settings.json` - Main app settings (audio, tts, twitch, webview, hotkey)
- `windows.json` - Window settings (main, floating, soundpanel)

**Old Files (Deprecated & Backed Up):**
- `audio_settings.json` → now in `settings.audio`
- `twitch/settings.json` → now in `settings.twitch`
- `webview/settings.json` → now in `settings.webview`
- `telegram_config.json` → now in `settings.tts.telegram.api_id`
- `soundpanel_appearance.json` → now in `windows.soundpanel`
- `settings.json` (old) → split into `settings` + `windows`

**Frontend Integration:**
- No changes needed to frontend code
- All Tauri commands maintain backward compatibility
- API signatures unchanged - frontend works as-is

**Migration Strategy:**
- Type A: Fresh start (no migration code)
- Old files backed up to `bak/old-config/`
- User manually restores settings if needed

## Notes

- ✅ The backend Rust code compiles successfully
- ✅ All config reorganization is complete
- ✅ All deprecated config files have been migrated
- ⏳ Frontend updates are pending
- ⏳ Some unused code warnings remain (can be cleaned up later)
