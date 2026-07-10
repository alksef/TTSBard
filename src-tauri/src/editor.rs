use std::sync::Arc;
use parking_lot::Mutex;
use crate::preprocessor::TextPreprocessor;
use crate::history::HistoryManager;
use crate::spellcheck::SpellcheckManager;

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

    pub fn reload_preprocessor(&self) {
        *self.preprocessor.lock() = TextPreprocessor::load_from_files().ok();
    }
}
