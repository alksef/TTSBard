//! Sound Panel Module
//!
//! Модуль для управления звуковой панелью - воспроизведение звуков
//! по горячим клавишам A-Z.

mod state;
mod storage;
mod audio;
mod bindings;
mod hook;

pub use state::SoundPanelState;
pub use storage::{load_bindings, load_appearance};
pub use hook::initialize_soundpanel_hook;

// Re-export Tauri commands
pub use bindings::{
    sp_get_bindings,
    sp_add_binding,
    sp_remove_binding,
    sp_test_sound,
    sp_is_supported_format,
    sp_get_floating_appearance,
    sp_set_floating_opacity,
    sp_set_floating_bg_color,
    sp_set_floating_clickthrough,
    sp_is_floating_clickthrough_enabled,
    sp_set_exclude_from_recording,
    sp_is_exclude_from_recording,
    sp_apply_exclude_recording,
};
