//! Sound Panel Module
//!
//! Модуль для управления звуковой панелью - воспроизведение звуков
//! по горячим клавишам A-Z.

mod audio;
mod bindings;
mod hook;
pub mod intercept;
mod state;
mod storage;

pub use hook::{initialize_soundpanel_hook, HookManager};
pub use state::{SoundBinding, SoundPanelState};
pub use storage::{load_appearance, load_bindings};

// Re-export Tauri commands
pub use bindings::{
    clear_intercept_binding, get_intercept_settings, set_intercept_binding, set_intercept_enabled,
    sp_add_binding, sp_add_set, sp_get_active_set, sp_get_bindings, sp_get_floating_appearance,
    sp_get_sets, sp_get_stay_visible, sp_is_floating_clickthrough_enabled, sp_is_supported_format,
    sp_play_binding, sp_remove_binding, sp_remove_set, sp_rename_set, sp_set_active_set,
    sp_set_floating_bg_color, sp_set_floating_clickthrough, sp_set_floating_opacity,
    sp_set_hide_on_blur, sp_set_stay_visible, sp_test_sound,
};
