//! Sound Panel Module
//!
//! Модуль для управления звуковой панелью - воспроизведение звуков
//! по горячим клавишам A-Z.

mod audio;
mod bindings;
mod hook;
mod state;
mod storage;

pub use hook::initialize_soundpanel_hook;
pub use state::{SoundBinding, SoundPanelState};
pub use storage::{load_appearance, load_bindings};

// Re-export Tauri commands
pub use bindings::{
    sp_add_binding, sp_get_bindings, sp_get_floating_appearance,
    sp_is_floating_clickthrough_enabled, sp_is_supported_format, sp_play_binding,
    sp_remove_binding, sp_set_floating_bg_color, sp_set_floating_clickthrough,
    sp_set_floating_opacity, sp_test_sound,
};
