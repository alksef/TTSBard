use crate::tabs::{TabManager, TabsData};
use std::sync::Arc;
use tauri::State;

pub struct TabsState(pub Arc<TabManager>);

#[tauri::command]
pub fn get_tabs(state: State<'_, TabsState>) -> TabsData {
    state.0.load_all()
}

#[tauri::command]
pub fn save_tabs(state: State<'_, TabsState>, data: TabsData) -> Result<(), String> {
    state.0.save_all(data);
    Ok(())
}
