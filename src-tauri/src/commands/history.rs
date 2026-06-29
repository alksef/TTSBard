use crate::history::{HistoryEntry, HistoryManager, PhraseEntry, PhraseSuggestion};
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

#[tauri::command]
pub fn get_phrase_history(
    filter: Option<String>,
    limit: Option<usize>,
    history_state: State<'_, HistoryState>,
) -> Result<Vec<PhraseEntry>, String> {
    let limit = limit.unwrap_or(100);
    let manager = &history_state.0;
    Ok(manager.get_phrases(filter.as_deref(), limit))
}

#[tauri::command]
pub fn delete_phrase_history(
    id: String,
    history_state: State<'_, HistoryState>,
) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("Phrase id cannot be empty".to_string());
    }
    let manager = &history_state.0;
    manager.delete_phrase(&id);
    Ok(())
}

#[tauri::command]
pub fn clear_phrase_history(history_state: State<'_, HistoryState>) -> Result<(), String> {
    let manager = &history_state.0;
    manager.clear_phrases();
    Ok(())
}
