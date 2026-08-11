use crate::spellcheck::{SpellResult, SpellcheckManager};
use std::sync::Arc;
use tauri::State;

pub struct SpellcheckState(pub Arc<SpellcheckManager>);

#[tauri::command]
pub fn spellcheck(
    words: Vec<String>,
    state: State<'_, SpellcheckState>,
) -> Result<Vec<SpellResult>, String> {
    if !state.0.is_available() {
        return Err("dictionary unavailable".into());
    }
    Ok(state.0.check_words(&words))
}
