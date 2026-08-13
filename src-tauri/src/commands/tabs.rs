use crate::tabs::{TabManager, TabsData};
use std::sync::Arc;
use tauri::State;

pub struct TabsState(pub Arc<TabManager>);

const TABS_PERSIST_FAILED: &str = "tabs.persist_failed";

#[tauri::command]
pub fn get_tabs(state: State<'_, TabsState>) -> TabsData {
    state.0.load_all()
}

#[tauri::command]
pub fn save_tabs(state: State<'_, TabsState>, data: TabsData) -> Result<(), String> {
    state.0.save_all(data).map_err(|e| {
        tracing::error!("Failed to persist tabs: {:#}", e);
        TABS_PERSIST_FAILED.to_string()
    })
}
