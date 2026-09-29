use crate::history::HistoryManager;
use crate::preprocessor::TextPreprocessor;
use crate::spellcheck::SpellcheckManager;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct EditorService {
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,
    pub history_manager: Arc<Mutex<Option<Arc<HistoryManager>>>>,
    pub spellcheck_manager: Arc<Mutex<Option<Arc<SpellcheckManager>>>>,
}

impl EditorService {
    pub fn new() -> Self {
        Self {
            preprocessor: Arc::new(Mutex::new(None)),
            history_manager: Arc::new(Mutex::new(None)),
            spellcheck_manager: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_preprocessor(&self) -> Option<TextPreprocessor> {
        let mut prep = self.preprocessor.lock();
        if prep.is_none() {
            *prep = TextPreprocessor::load_from_files().ok();
        }
        prep.clone()
    }

    /// Owned handle to the phrase-history manager.
    ///
    /// The slot guard is released here, so callers never hold a `parking_lot`
    /// guard across blocking work or `.await` (see `DECISION-018`).
    pub fn history_handle(&self) -> Option<Arc<HistoryManager>> {
        self.history_manager.lock().as_ref().cloned()
    }

    pub fn reload_preprocessor(&self) {
        *self.preprocessor.lock() = TextPreprocessor::load_from_files().ok();
    }
}
