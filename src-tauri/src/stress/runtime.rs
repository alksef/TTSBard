//! Explicit native RUAccent runtime slot.
//!
//! A slot owns a discovered native-ready pack descriptor plus a thread-safe,
//! Clone-friendly diagnostic status. Producing or cloning a slot never creates
//! an ONNX session and never reads model bytes. A runtime is built only by the
//! explicit [`RuAccentRuntimeSlot::load_or_retry`] operation (intended to run
//! inside `spawn_blocking`) or by the startup loader. Annotating a text never
//! loads a model: it succeeds only for an already-ready runtime.

use crate::stress::annotations::StructuredStress;
use crate::stress::packs::RuAccentPackDescriptor;
use parking_lot::Mutex;
use ruaccent_rs::RuAccent;
use std::sync::Arc;

/// Safe, Clone-friendly diagnostic status of the native RUAccent runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuAccentRuntimeStatus {
    NotLoaded,
    Loading,
    Ready,
    Failed { message: String },
}

impl RuAccentRuntimeStatus {
    /// The safe wire string for this status. `Failed` loses its message.
    pub fn as_safe_str(&self) -> &'static str {
        match self {
            RuAccentRuntimeStatus::NotLoaded => "not_loaded",
            RuAccentRuntimeStatus::Loading => "loading",
            RuAccentRuntimeStatus::Ready => "ready",
            RuAccentRuntimeStatus::Failed { .. } => "failed",
        }
    }
}

/// A safe not-ready error returned by [`RuAccentRuntimeSlot::annotate`] when no
/// runtime is resident. It never carries paths or ONNX details.
const NOT_READY_ERROR: &str = "Модель RUAccent не загружена";

/// A loaded native pipeline that annotates stress positions with `+` markers.
///
/// The trait boundary keeps the slot's load-once / reuse behavior testable
/// without requiring real ONNX model files.
trait MarkedAnnotator: Send {
    fn annotate_marked(&mut self, text: &str) -> String;
}

impl MarkedAnnotator for RuAccent {
    fn annotate_marked(&mut self, text: &str) -> String {
        self.process_all(text)
    }
}

/// Owns a pack descriptor and a shared, explicitly evolving status and runtime.
///
/// Cloning the slot shares the underlying status and runtime (`Arc`), so state
/// transitions and the single loaded runtime are visible to all holders.
/// Exposing `status()` and `descriptor()` is side-effect free: no sessions are
/// created and no model bytes are read.
#[derive(Clone)]
pub struct RuAccentRuntimeSlot {
    descriptor: RuAccentPackDescriptor,
    status: Arc<Mutex<RuAccentRuntimeStatus>>,
    runtime: Arc<Mutex<Option<Box<dyn MarkedAnnotator>>>>,
}

impl std::fmt::Debug for RuAccentRuntimeSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuAccentRuntimeSlot")
            .field("descriptor", &self.descriptor)
            .field("status", &self.status())
            .field("runtime_loaded", &self.runtime.lock().is_some())
            .finish()
    }
}

impl RuAccentRuntimeSlot {
    /// Create a slot for a discovered pack. The constructor never touches the
    /// filesystem.
    pub fn new(descriptor: RuAccentPackDescriptor) -> Self {
        Self {
            descriptor,
            status: Arc::new(Mutex::new(RuAccentRuntimeStatus::NotLoaded)),
            runtime: Arc::new(Mutex::new(None)),
        }
    }

    /// Current diagnostic status. Never creates sessions or reads model bytes.
    pub fn status(&self) -> RuAccentRuntimeStatus {
        self.status.lock().clone()
    }

    /// Whether the slot currently has a ready runtime.
    pub fn is_ready(&self) -> bool {
        matches!(self.status(), RuAccentRuntimeStatus::Ready)
    }

    /// The pack descriptor backing this slot. Never creates sessions or reads
    /// model bytes.
    pub fn descriptor(&self) -> &RuAccentPackDescriptor {
        &self.descriptor
    }

    /// Annotate stress positions with the resident native `ruaccent_rs`
    /// pipeline.
    ///
    /// This never loads a model: it succeeds only for an already-ready runtime
    /// and otherwise returns a safe not-ready error. A per-input output
    /// validation mismatch is a request error — it is returned to the caller
    /// and never marks the runtime `Failed`, so a later valid inference keeps
    /// the status `Ready`.
    pub fn annotate(&self, text: &str) -> Result<StructuredStress, String> {
        let marked = self.run_native(text)?;
        crate::stress::contextual::structured_stress_from_marked(text, &marked)
            .map_err(|error| format!("RUAccent native runtime produced invalid output: {error}"))
    }

    /// Run one text through the shared runtime, which must already be loaded.
    fn run_native(&self, text: &str) -> Result<String, String> {
        let mut guard = self.runtime.lock();
        let runtime = guard.as_mut().ok_or_else(|| NOT_READY_ERROR.to_string())?;
        Ok(runtime.annotate_marked(text))
    }

    /// Synchronously load the runtime or retry a previous failed load.
    ///
    /// Intended to run inside `spawn_blocking`. Transitions `NotLoaded` or
    /// `Failed` to `Loading` and then to `Ready` or `Failed`. Concurrent
    /// attempts are serialized on the runtime mutex, and the operation returns
    /// immediately when a runtime is already resident.
    pub fn load_or_retry(&self) -> Result<(), String> {
        if self.is_ready() {
            return Ok(());
        }
        let mut guard = self.runtime.lock();
        if guard.is_some() {
            self.mark_ready();
            return Ok(());
        }
        self.load_native(&mut guard)
    }

    /// Release the resident runtime and reset the slot to `NotLoaded`.
    pub fn unload(&self) {
        *self.runtime.lock() = None;
        *self.status.lock() = RuAccentRuntimeStatus::NotLoaded;
    }

    /// Transition to `Ready` once the native runtime has been built. Intended
    /// for the loading path.
    pub fn mark_ready(&self) {
        *self.status.lock() = RuAccentRuntimeStatus::Ready;
    }

    /// Transition to `Failed` with a diagnostic message. The message must never
    /// expose absolute filesystem paths; callers are responsible for sanitizing
    /// it (e.g. via `crate::secret_log::safe_path_for_log`).
    pub fn mark_failed(&self, message: String) {
        *self.status.lock() = RuAccentRuntimeStatus::Failed { message };
    }

    /// Build the single `RuAccent` from the pack root and omograph model id,
    /// caching it in `guard`.
    ///
    /// Status moves `Loading` then `Ready` on success or `Failed` with a
    /// path-safe diagnostic on failure. The caller already holds the runtime
    /// mutex, so only one load can run at a time.
    fn load_native(&self, guard: &mut Option<Box<dyn MarkedAnnotator>>) -> Result<(), String> {
        *self.status.lock() = RuAccentRuntimeStatus::Loading;
        let result = RuAccent::load_with_omograph(
            &self.descriptor.pack_root,
            &self.descriptor.omograph_model_id,
        );
        match result {
            Ok(runtime) => {
                *guard = Some(Box::new(runtime));
                self.mark_ready();
                Ok(())
            }
            Err(error) => {
                let message = format!(
                    "RUAccent native runtime failed to load: {}",
                    safe_load_error(&error.to_string(), &self.descriptor.pack_root)
                );
                self.mark_failed(message.clone());
                Err(message)
            }
        }
    }
}

/// Mask the pack root inside a load error so the diagnostic never exposes an
/// absolute filesystem path.
///
/// Most `ruaccent_rs` errors carry file names only, but the underlying `ort`
/// session builder embeds the full model path for a missing file. Stripping the
/// pack root keeps the useful file name while hiding the user path.
fn safe_load_error(error: &str, pack_root: &std::path::Path) -> String {
    let root = pack_root.display().to_string();
    let root_forward = root.replace('\\', "/");
    let mut sanitized = error.replace(&root, "[PACK]");
    if root_forward != root {
        sanitized = sanitized.replace(&root_forward, "[PACK]");
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stress::annotations::StressAnnotation;
    use crate::stress::packs::RuntimeCapability;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    fn descriptor() -> RuAccentPackDescriptor {
        RuAccentPackDescriptor {
            id: "com.example.slot".to_string(),
            display_name: "Slot".to_string(),
            runtime_version: "upstream".to_string(),
            pack_root: PathBuf::from("/does/not/exist"),
            runtime_capability: RuntimeCapability::NativeTiny,
            omograph_model_id: "slot-model".to_string(),
        }
    }

    fn native_slot(pack_root: PathBuf) -> RuAccentRuntimeSlot {
        let descriptor = RuAccentPackDescriptor {
            id: "com.example.native".to_string(),
            display_name: "Native".to_string(),
            runtime_version: "upstream".to_string(),
            pack_root,
            runtime_capability: RuntimeCapability::NativeTiny,
            omograph_model_id: "native-model".to_string(),
        };
        RuAccentRuntimeSlot::new(descriptor)
    }

    fn test_directory() -> PathBuf {
        let number = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "ttsbard-ruaccent-runtime-{}-{number}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    /// A test pipeline that returns a fixed marked output per input and counts
    /// how many times it has been invoked.
    struct FakePipeline {
        calls: Arc<AtomicUsize>,
        outputs: HashMap<String, String>,
    }

    impl FakePipeline {
        fn new(outputs: HashMap<String, String>) -> (Self, Arc<AtomicUsize>) {
            let calls = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    calls: Arc::clone(&calls),
                    outputs,
                },
                calls,
            )
        }
    }

    impl MarkedAnnotator for FakePipeline {
        fn annotate_marked(&mut self, text: &str) -> String {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outputs
                .get(text)
                .cloned()
                .unwrap_or_else(|| text.to_string())
        }
    }

    /// Install a fake runtime and mark the slot ready.
    fn inject_fake(
        slot: &RuAccentRuntimeSlot,
        outputs: HashMap<String, String>,
    ) -> Arc<AtomicUsize> {
        let (fake, calls) = FakePipeline::new(outputs);
        *slot.runtime.lock() = Some(Box::new(fake));
        slot.mark_ready();
        calls
    }

    #[test]
    fn status_starts_not_loaded() {
        let slot = RuAccentRuntimeSlot::new(descriptor());
        assert_eq!(slot.status(), RuAccentRuntimeStatus::NotLoaded);
        assert!(!slot.is_ready());
    }

    #[test]
    fn mark_ready_and_failed_transitions() {
        let slot = RuAccentRuntimeSlot::new(descriptor());

        slot.mark_ready();
        assert_eq!(slot.status(), RuAccentRuntimeStatus::Ready);
        assert_eq!(slot.status().as_safe_str(), "ready");

        slot.mark_failed("load failed".to_string());
        assert_eq!(
            slot.status(),
            RuAccentRuntimeStatus::Failed {
                message: "load failed".to_string()
            }
        );
        assert_eq!(slot.status().as_safe_str(), "failed");
    }

    #[test]
    fn safe_status_strings_are_preserved() {
        let slot = RuAccentRuntimeSlot::new(descriptor());
        assert_eq!(slot.status().as_safe_str(), "not_loaded");

        *slot.status.lock() = RuAccentRuntimeStatus::Loading;
        assert_eq!(slot.status().as_safe_str(), "loading");
    }

    #[test]
    fn status_and_descriptor_never_touch_filesystem() {
        let slot = RuAccentRuntimeSlot::new(descriptor());
        assert_eq!(slot.descriptor().id, "com.example.slot");
        assert_eq!(slot.descriptor().omograph_model_id, "slot-model");
        assert_eq!(slot.status(), RuAccentRuntimeStatus::NotLoaded);
    }

    #[test]
    fn cloned_slot_shares_status() {
        let slot = RuAccentRuntimeSlot::new(descriptor());
        let clone = slot.clone();
        clone.mark_ready();
        assert_eq!(slot.status(), RuAccentRuntimeStatus::Ready);
    }

    #[test]
    fn native_annotate_converts_marked_result() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        inject_fake(
            &slot,
            HashMap::from([("замок и замок".to_string(), "з+амок и зам+ок".to_string())]),
        );

        let stress = slot.annotate("замок и замок").unwrap();
        assert_eq!(
            stress.annotations,
            vec![
                StressAnnotation {
                    word_start: 0,
                    word_end: 10,
                    stressed_vowel: 2
                },
                StressAnnotation {
                    word_start: 14,
                    word_end: 24,
                    stressed_vowel: 20
                },
            ]
        );
        assert_eq!(stress.original, "замок и замок");
    }

    #[test]
    fn native_annotate_reuses_loaded_runtime() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        let calls = inject_fake(
            &slot,
            HashMap::from([("замок".to_string(), "зам+ок".to_string())]),
        );

        assert_eq!(slot.annotate("замок").unwrap().annotations.len(), 1);
        assert_eq!(slot.annotate("замок").unwrap().annotations.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn annotate_without_ready_runtime_returns_safe_error_without_loading() {
        let directory = test_directory();
        let slot = native_slot(directory.clone());

        // No load has run: annotate must not attempt to load the model.
        let error = slot.annotate("привет").unwrap_err();
        assert_eq!(error, NOT_READY_ERROR);
        assert!(
            !error.contains('/') && !error.contains('\\'),
            "leaked path: {error}"
        );
        assert_eq!(slot.status(), RuAccentRuntimeStatus::NotLoaded);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn annotate_rejects_dangling_marker_without_marking_failed() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        inject_fake(
            &slot,
            HashMap::from([("привет".to_string(), "привет+".to_string())]),
        );

        let error = slot.annotate("привет").unwrap_err();
        assert!(
            !error.contains('/') && !error.contains('\\'),
            "leaked path: {error}"
        );
        assert_eq!(slot.status(), RuAccentRuntimeStatus::Ready);
    }

    #[test]
    fn annotate_rejects_misaligned_output_without_marking_failed() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        inject_fake(
            &slot,
            HashMap::from([("привет".to_string(), "пока".to_string())]),
        );

        let error = slot.annotate("привет").unwrap_err();
        assert!(
            error.contains("invalid output"),
            "unexpected error: {error}"
        );
        assert_eq!(slot.status(), RuAccentRuntimeStatus::Ready);
    }

    #[test]
    fn load_or_retry_failure_marks_failed_without_panic() {
        let directory = test_directory();
        let slot = native_slot(directory.clone());

        let error = slot.load_or_retry().unwrap_err();
        assert!(
            !error.contains(directory.to_string_lossy().as_ref()),
            "leaked path: {error}"
        );
        assert_eq!(
            slot.status(),
            RuAccentRuntimeStatus::Failed { message: error }
        );

        // A failed load is memoized: annotate reports not-ready and never retries.
        assert_eq!(slot.annotate("привет").unwrap_err(), NOT_READY_ERROR);
        assert!(matches!(
            slot.status(),
            RuAccentRuntimeStatus::Failed { .. }
        ));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn load_or_retry_transitions_loading_to_ready() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        // A fake runtime is pre-installed but status is NotLoaded; load_or_retry
        // should recognize the resident runtime and become Ready.
        let (fake, _) = FakePipeline::new(HashMap::new());
        *slot.runtime.lock() = Some(Box::new(fake));

        assert!(slot.load_or_retry().is_ok());
        assert_eq!(slot.status(), RuAccentRuntimeStatus::Ready);
    }

    #[test]
    fn unload_resets_runtime_and_status() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        inject_fake(
            &slot,
            HashMap::from([("замок".to_string(), "зам+ок".to_string())]),
        );
        assert!(slot.is_ready());

        slot.unload();
        assert_eq!(slot.status(), RuAccentRuntimeStatus::NotLoaded);
        assert_eq!(slot.annotate("замок").unwrap_err(), NOT_READY_ERROR);
    }

    #[test]
    fn load_or_retry_returns_immediately_when_ready() {
        let slot = native_slot(PathBuf::from("/does/not/exist"));
        let calls = inject_fake(
            &slot,
            HashMap::from([("замок".to_string(), "зам+ок".to_string())]),
        );

        assert!(slot.load_or_retry().is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn safe_load_error_masks_pack_root() {
        let root = std::path::Path::new(r"C:\Users\Al\models\pack");
        let error = r"File at `C:\Users\Al\models\pack\nn\nn_accent\model.onnx` does not exist";
        let sanitized = safe_load_error(error, root);
        assert!(!sanitized.contains("Users"), "leaked path: {sanitized}");
        assert!(sanitized.contains("[PACK]"));
        assert!(
            sanitized.contains("model.onnx"),
            "useful file name lost: {sanitized}"
        );
    }

    #[test]
    fn safe_load_error_masks_forward_slash_pack_root() {
        let root = std::path::Path::new(r"C:\Users\Al\models\pack");
        let error = r"File at `C:/Users/Al/models/pack/nn/nn_accent/model.onnx` does not exist";
        let sanitized = safe_load_error(error, root);
        assert!(!sanitized.contains("Users"), "leaked path: {sanitized}");
    }
}
