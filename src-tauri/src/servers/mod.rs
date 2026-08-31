// Servers module - Network server management
//
// This module manages WebView, Twitch and Input servers for the application.
// Refactored from lib.rs server threads (2026-03-11)

mod input_server;
mod twitch;
mod webview;

// Re-export server runner functions for use in setup.rs
pub use input_server::run_input_server;
pub use twitch::run_twitch_client;
pub use webview::run_webview_server;
