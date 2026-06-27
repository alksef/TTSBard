use crate::history::{HistoryEntry, HistoryManager, PhraseSuggestion};
use std::sync::Arc;
use tauri::State;

pub struct HistoryState(pub Arc<HistoryManager>);

#[tauri::command]
pub fn get_history_suggestions(
    query: String,
    limit: Option<usize>,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<HistoryEntry>, String> {
    let limit = limit.unwrap_or(10);
    let manager = &history_state.0;
    Ok(manager.suggest(&query, limit))
}

#[tauri::command]
pub fn get_phrase_completion(
    context: String,
    limit: Option<usize>,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<PhraseSuggestion>, String> {
    let limit = limit.unwrap_or(5);
    let manager = &history_state.0;
    Ok(manager.suggest_phrase(&context, limit))
}

#[tauri::command]
pub fn record_history(text: String, history_state: State<'_, HistoryState>) {
    let manager = &history_state.0;
    manager.record_text(&text);
}

#[tauri::command]
pub fn clear_history(history_state: State<'_, HistoryState>) {
    let manager = &history_state.0;
    manager.clear();
}
