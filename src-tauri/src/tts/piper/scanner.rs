use std::path::{Path, PathBuf};

/// Descriptor for a discovered Piper voice model.
///
/// Each valid `.onnx` + `.onnx.json` pair produces one descriptor with a stable
/// provider ID that can be stored in settings and matched across restarts.
#[derive(Debug, Clone)]
pub struct PiperModelDescriptor {
    pub id: String,
    pub display_name: String,
    pub onnx_path: PathBuf,
    pub json_path: PathBuf,
    // Validated at discovery time and retained for future model-detail UI.
    #[allow(dead_code)]
    pub sample_rate: u32,
    #[allow(dead_code)]
    pub phoneme_id_map: serde_json::Value,
}

const MODELS_SUBDIR: &str = "models/piper";
const ID_PREFIX: &str = "local-piper";

/// Discover Piper models under `root/models/piper`, creating the directory if
/// needed. Only used for writable roots (the local data root).
pub fn discover_piper_models(root: &Path) -> Vec<PiperModelDescriptor> {
    let models_dir = root.join(MODELS_SUBDIR);

    if let Err(e) = std::fs::create_dir_all(&models_dir) {
        tracing::warn!(
            dir = %crate::secret_log::safe_path_for_log(&models_dir),
            error = %e,
            "Failed to create Piper models directory"
        );
        return Vec::new();
    }

    scan_piper_models_dir(&models_dir)
}

/// Scan `models/piper` under a root directory without creating anything.
///
/// For read-only roots (the process cwd): a missing directory is normal
/// and stays silent; the root directory is never created.
pub fn scan_piper_models_root(root: &Path) -> Vec<PiperModelDescriptor> {
    scan_piper_models_dir(&root.join(MODELS_SUBDIR))
}

/// Merge validated per-root descriptor lists; the earlier root wins on a
/// duplicate ID. Inputs are already validated per root, so a broken candidate
/// in an earlier root never appears here and cannot block a valid same-ID
/// model in a later root. The merged list is sorted by ID for a deterministic
/// order regardless of which root each model came from.
pub(crate) fn merge_piper_roots(
    per_root: Vec<Vec<PiperModelDescriptor>>,
) -> Vec<PiperModelDescriptor> {
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();
    for descriptors in per_root {
        for descriptor in descriptors {
            if seen.insert(descriptor.id.clone()) {
                merged.push(descriptor);
            }
        }
    }
    merged.sort_by(|a, b| a.id.cmp(&b.id));
    merged
}

/// Scan a concrete `models/piper` directory without side effects.
///
/// A missing directory is normal for read-only roots (e.g. next to the exe)
/// and stays silent; other read failures are logged.
pub fn scan_piper_models_dir(models_dir: &Path) -> Vec<PiperModelDescriptor> {
    let entries = match std::fs::read_dir(models_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!(
                dir = %crate::secret_log::safe_path_for_log(models_dir),
                error = %e,
                "Failed to read Piper models directory"
            );
            return Vec::new();
        }
    };

    let mut results = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("onnx") {
            continue;
        }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => {
                let lossy = path.to_string_lossy();
                tracing::warn!(
                    path = %lossy,
                    "Skipping ONNX file with non-UTF-8 filename"
                );
                continue;
            }
        };

        let json_path = path.with_extension("onnx.json");
        if !json_path.is_file() {
            tracing::warn!(
                onnx = %crate::secret_log::safe_path_for_log(&path),
                "Skipping ONNX file without corresponding .onnx.json"
            );
            continue;
        }

        let json_content = match std::fs::read_to_string(&json_path) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(
                    json = %crate::secret_log::safe_path_for_log(&json_path),
                    error = %e,
                    "Failed to read Piper JSON config"
                );
                continue;
            }
        };

        let config: serde_json::Value = match serde_json::from_str(&json_content) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    json = %crate::secret_log::safe_path_for_log(&json_path),
                    error = %e,
                    "Failed to parse Piper JSON config"
                );
                continue;
            }
        };

        let sample_rate = match parse_sample_rate(&config) {
            Some(rate) => rate,
            None => {
                tracing::warn!(
                    json = %crate::secret_log::safe_path_for_log(&json_path),
                    "Piper JSON missing or invalid audio.sample_rate"
                );
                continue;
            }
        };

        let phoneme_id_map = match config.get("phoneme_id_map") {
            Some(map) if map.is_object() => map.clone(),
            Some(_) => {
                tracing::warn!(
                    json = %crate::secret_log::safe_path_for_log(&json_path),
                    "Piper JSON phoneme_id_map is not an object"
                );
                continue;
            }
            None => {
                tracing::warn!(
                    json = %crate::secret_log::safe_path_for_log(&json_path),
                    "Piper JSON missing phoneme_id_map"
                );
                continue;
            }
        };

        let id = format!("{}:{}", ID_PREFIX, stem);
        let display_name = stem_to_display_name(stem);

        tracing::info!(
            id = %id,
            display = %display_name,
            sample_rate = sample_rate,
            phonemes = phoneme_id_map.as_object().map(|m| m.len()).unwrap_or(0),
            "Discovered Piper model"
        );

        results.push(PiperModelDescriptor {
            id,
            display_name,
            onnx_path: path,
            json_path,
            sample_rate,
            phoneme_id_map,
        });
    }

    results.sort_by(|a, b| a.id.cmp(&b.id));

    let count = results.len();
    if count > 0 {
        tracing::info!(count = count, "Piper model discovery complete");
    }

    results
}

fn parse_sample_rate(config: &serde_json::Value) -> Option<u32> {
    let rate = config.get("audio")?.get("sample_rate")?.as_u64()?;
    if rate == 0 || rate > u32::MAX as u64 {
        return None;
    }
    Some(rate as u32)
}

fn stem_to_display_name(stem: &str) -> String {
    stem.replace(['_', '-'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn unique_test_root(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ttsbard-piper-test-{}-{}-{}",
            std::process::id(),
            unique,
            name
        ))
    }

    fn create_model_pair(root: &Path, stem: &str, sample_rate: u32) {
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();

        let onnx_path = models_dir.join(format!("{}.onnx", stem));
        let json_path = models_dir.join(format!("{}.onnx.json", stem));

        std::fs::write(&onnx_path, b"dummy onnx content").unwrap();

        let json_content = serde_json::json!({
            "audio": {
                "sample_rate": sample_rate
            },
            "phoneme_id_map": {
                "_": [0],
                "^": [1],
                "$": [2],
                " ": [3],
                "a": [4],
                "b": [5]
            }
        });
        std::fs::write(&json_path, json_content.to_string()).unwrap();
    }

    fn create_onnx_only(root: &Path, stem: &str) {
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();
        let onnx_path = models_dir.join(format!("{}.onnx", stem));
        std::fs::write(&onnx_path, b"dummy onnx content").unwrap();
    }

    fn create_json_only(root: &Path, stem: &str, sample_rate: u32) {
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();
        let json_path = models_dir.join(format!("{}.onnx.json", stem));
        let json_content = serde_json::json!({
            "audio": { "sample_rate": sample_rate },
            "phoneme_id_map": { "a": [0] }
        });
        std::fs::write(&json_path, json_content.to_string()).unwrap();
    }

    fn create_invalid_json(root: &Path, stem: &str) {
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();
        let onnx_path = models_dir.join(format!("{}.onnx", stem));
        let json_path = models_dir.join(format!("{}.onnx.json", stem));
        std::fs::write(&onnx_path, b"dummy onnx").unwrap();
        std::fs::write(&json_path, b"not valid json {{{").unwrap();
    }

    fn create_json_missing_sample_rate(root: &Path, stem: &str) {
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();
        let onnx_path = models_dir.join(format!("{}.onnx", stem));
        let json_path = models_dir.join(format!("{}.onnx.json", stem));
        std::fs::write(&onnx_path, b"dummy onnx").unwrap();
        let json_content = serde_json::json!({
            "phoneme_id_map": { "a": [0] }
        });
        std::fs::write(&json_path, json_content.to_string()).unwrap();
    }

    fn create_json_missing_phoneme_map(root: &Path, stem: &str) {
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();
        let onnx_path = models_dir.join(format!("{}.onnx", stem));
        let json_path = models_dir.join(format!("{}.onnx.json", stem));
        std::fs::write(&onnx_path, b"dummy onnx").unwrap();
        let json_content = serde_json::json!({
            "audio": { "sample_rate": 22050 }
        });
        std::fs::write(&json_path, json_content.to_string()).unwrap();
    }

    // --- Tests ---

    /// Empty/missing directory: scan should return empty, not panic.
    #[test]
    fn empty_directory_returns_none() {
        let root = unique_test_root("empty");
        std::fs::create_dir_all(&root).unwrap();
        // Do NOT create models/piper — it should be created.
        let models = discover_piper_models(&root);
        assert!(models.is_empty());

        let models_dir = root.join(MODELS_SUBDIR);
        assert!(models_dir.exists(), "models/piper should have been created");

        // Second scan of now-existing empty dir
        let models2 = discover_piper_models(&root);
        assert!(models2.is_empty());
    }

    /// Any root that doesn't exist should still succeed (dir created, empty result).
    #[test]
    fn missing_root_creates_dir_and_returns_empty() {
        let root = unique_test_root("missing-root");
        // root does not exist yet
        assert!(!root.exists());
        let models = discover_piper_models(&root);
        assert!(models.is_empty());
        assert!(root.join(MODELS_SUBDIR).exists());
    }

    /// Read-only scan (portable root next to the exe) must not create anything.
    #[test]
    fn scan_missing_dir_creates_nothing_and_is_empty() {
        let root = unique_test_root("scan-missing");
        assert!(!root.exists());
        let models = scan_piper_models_dir(&root.join(MODELS_SUBDIR));
        assert!(models.is_empty());
        assert!(!root.exists(), "read-only scan must not create directories");
    }

    /// Valid pair: one .onnx + .onnx.json produces exactly one descriptor.
    #[test]
    fn valid_pair_returns_descriptor() {
        let root = unique_test_root("valid");
        create_model_pair(&root, "ru_RU-irina-medium-cloned", 22050);

        let models = discover_piper_models(&root);
        assert_eq!(models.len(), 1);

        let m = &models[0];
        assert_eq!(m.id, "local-piper:ru_RU-irina-medium-cloned");
        assert_eq!(m.display_name, "ru RU irina medium cloned");
        assert!(m.onnx_path.ends_with("ru_RU-irina-medium-cloned.onnx"));
        assert!(m.json_path.ends_with("ru_RU-irina-medium-cloned.onnx.json"));
        assert_eq!(m.sample_rate, 22050);
        assert!(m.phoneme_id_map.is_object());
        assert_eq!(m.phoneme_id_map.as_object().unwrap().len(), 6);
    }

    /// Missing .onnx.json: ONNX should be skipped.
    #[test]
    fn missing_json_skipped() {
        let root = unique_test_root("missing-json");
        create_onnx_only(&root, "some-model");

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    /// Malformed JSON: pair should be skipped.
    #[test]
    fn malformed_json_skipped() {
        let root = unique_test_root("malformed-json");
        create_invalid_json(&root, "broken-model");

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    /// JSON without audio.sample_rate: pair should be skipped.
    #[test]
    fn missing_sample_rate_skipped() {
        let root = unique_test_root("missing-rate");
        create_json_missing_sample_rate(&root, "no-rate-model");

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    /// JSON without phoneme_id_map: pair should be skipped.
    #[test]
    fn missing_phoneme_map_skipped() {
        let root = unique_test_root("missing-phonemes");
        create_json_missing_phoneme_map(&root, "no-phoneme-model");

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    /// Deterministic ordering: results sorted by ID, not filesystem order.
    #[test]
    fn deterministic_ordering() {
        let root = unique_test_root("ordering");

        // Create in non-alphabetical order to verify sorting
        create_model_pair(&root, "zzz-last", 22050);
        create_model_pair(&root, "aaa-first", 22050);
        create_model_pair(&root, "mmm-middle", 22050);

        let models = discover_piper_models(&root);
        assert_eq!(models.len(), 3);

        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let expected: Vec<&str> = vec![
            "local-piper:aaa-first",
            "local-piper:mmm-middle",
            "local-piper:zzz-last",
        ];
        assert_eq!(ids, expected);
    }

    /// Multiple valid pairs are all discovered.
    #[test]
    fn multiple_valid_pairs() {
        let root = unique_test_root("multiple");
        create_model_pair(&root, "voice-a", 16000);
        create_model_pair(&root, "voice-b", 22050);

        let models = discover_piper_models(&root);
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.id == "local-piper:voice-a"));
        assert!(models.iter().any(|m| m.id == "local-piper:voice-b"));
    }

    /// JSON with zero sample_rate: skipped.
    #[test]
    fn zero_sample_rate_skipped() {
        let root = unique_test_root("zero-rate");
        create_model_pair(&root, "zero-model", 0);

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    /// Only .onnx.json files that have a matching .onnx are NOT discovered
    /// (orphan JSON ignored, not considered a model).
    #[test]
    fn orphan_json_not_discovered() {
        let root = unique_test_root("orphan-json");
        create_json_only(&root, "orphan", 22050);

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    /// Subdirectories inside models/piper are not recursed into.
    #[test]
    fn subdirectories_not_recursed() {
        let root = unique_test_root("subdirs");
        let nested = root.join(MODELS_SUBDIR).join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("nested-model.onnx"), b"dummy").unwrap();
        let nested_json = serde_json::json!({
            "audio": {"sample_rate": 22050},
            "phoneme_id_map": {"a": [0]}
        });
        std::fs::write(
            nested.join("nested-model.onnx.json"),
            nested_json.to_string(),
        )
        .unwrap();

        let models = discover_piper_models(&root);
        assert!(models.is_empty(), "nested models should not be discovered");
    }

    /// parse_sample_rate rejects sample_rate above u32::MAX.
    #[test]
    fn parse_sample_rate_too_large() {
        let cfg = serde_json::json!({"audio": {"sample_rate": 4_294_967_296u64}});
        assert!(parse_sample_rate(&cfg).is_none());
    }

    /// parse_sample_rate rejects sample_rate of 0.
    #[test]
    fn parse_sample_rate_zero() {
        let cfg = serde_json::json!({"audio": {"sample_rate": 0}});
        assert!(parse_sample_rate(&cfg).is_none());
    }

    /// parse_sample_rate returns valid rate.
    #[test]
    fn parse_sample_rate_valid() {
        let cfg = serde_json::json!({"audio": {"sample_rate": 22050}});
        assert_eq!(parse_sample_rate(&cfg), Some(22050));
    }

    /// stem_to_display_name replaces underscore and hyphen with space.
    #[test]
    fn stem_to_display_name_basic() {
        assert_eq!(
            stem_to_display_name("ru_RU-irina-medium"),
            "ru RU irina medium"
        );
    }

    /// stem_to_display_name with no separators.
    #[test]
    fn stem_to_display_name_no_separators() {
        assert_eq!(stem_to_display_name("simple"), "simple");
    }

    /// Phoneme_id_map that is an array (not object) is skipped.
    #[test]
    fn phoneme_map_array_skipped() {
        let root = unique_test_root("array-phoneme");
        let models_dir = root.join(MODELS_SUBDIR);
        std::fs::create_dir_all(&models_dir).unwrap();
        std::fs::write(models_dir.join("bad.onnx"), b"dummy").unwrap();
        let json = serde_json::json!({
            "audio": {"sample_rate": 22050},
            "phoneme_id_map": ["a", "b", "c"]
        });
        std::fs::write(models_dir.join("bad.onnx.json"), json.to_string()).unwrap();

        let models = discover_piper_models(&root);
        assert!(models.is_empty());
    }

    // --- Two-root discovery (cwd → data root, ROADMAP-117) ---

    fn descriptor(id: &str, root: &Path) -> PiperModelDescriptor {
        PiperModelDescriptor {
            id: id.to_string(),
            display_name: id.to_string(),
            onnx_path: root.join(format!("{id}.onnx")),
            json_path: root.join(format!("{id}.onnx.json")),
            sample_rate: 22050,
            phoneme_id_map: serde_json::json!({"a": [0]}),
        }
    }

    /// Read-only root scan never creates `models/piper` (cwd must stay clean).
    #[test]
    fn scan_piper_models_root_creates_nothing() {
        let root = unique_test_root("scan-root-readonly");
        std::fs::create_dir_all(&root).unwrap();

        let models = scan_piper_models_root(&root);

        assert!(models.is_empty());
        assert!(
            !root.join("models").exists(),
            "read-only root scan must not create the models directory"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn merge_piper_roots_empty_inputs_yield_empty() {
        assert!(merge_piper_roots(vec![Vec::new(), Vec::new()]).is_empty());
        assert!(merge_piper_roots(Vec::new()).is_empty());
    }

    /// Different valid IDs from both roots stay available together.
    #[test]
    fn merge_piper_roots_keeps_ids_from_both_roots() {
        let cwd = unique_test_root("merge-both-cwd");
        let data = unique_test_root("merge-both-data");

        let merged = merge_piper_roots(vec![
            vec![descriptor("local-piper:cwd-only", &cwd)],
            vec![descriptor("local-piper:data-only", &data)],
        ]);

        let ids: Vec<&str> = merged.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["local-piper:cwd-only", "local-piper:data-only"]);
    }

    /// The same valid ID in both roots: the earlier root (cwd) wins.
    #[test]
    fn merge_piper_roots_earlier_root_wins_duplicate_id() {
        let cwd = unique_test_root("merge-dup-cwd");
        let data = unique_test_root("merge-dup-data");

        let merged = merge_piper_roots(vec![
            vec![descriptor("local-piper:dup", &cwd)],
            vec![descriptor("local-piper:dup", &data)],
        ]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].onnx_path, cwd.join("local-piper:dup.onnx"));
    }

    /// Recursive listing of every file and directory under `root`, for
    /// read-only guarantees in two-root tests.
    fn fs_listing(root: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path.clone());
                }
                paths.push(path);
            }
        }
        paths.sort();
        paths
    }

    /// An invalid cwd candidate (scan yields nothing) cannot block the valid
    /// same-ID model in the data root.
    #[test]
    fn merge_piper_roots_invalid_earlier_root_allows_fallback() {
        let cwd = unique_test_root("merge-fallback-cwd");
        let data = unique_test_root("merge-fallback-data");
        create_onnx_only(&cwd, "shared-model");
        create_model_pair(&data, "shared-model", 22050);

        let before = fs_listing(&cwd);
        let merged = merge_piper_roots(vec![
            scan_piper_models_dir(&cwd.join(MODELS_SUBDIR)),
            discover_piper_models(&data),
        ]);
        assert_eq!(
            fs_listing(&cwd),
            before,
            "read-only cwd scan must not change the directory"
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "local-piper:shared-model");
        assert!(
            merged[0].onnx_path.starts_with(&data),
            "fallback descriptor must come from the data root"
        );
    }

    /// The same root passed twice (cwd equals the data root) is not scanned
    /// into duplicates.
    #[test]
    fn merge_piper_roots_identical_roots_deduplicate() {
        let root = unique_test_root("merge-same-root");
        create_model_pair(&root, "voice", 22050);

        let models_dir_scan = || scan_piper_models_dir(&root.join(MODELS_SUBDIR));
        let merged = merge_piper_roots(vec![models_dir_scan(), models_dir_scan()]);

        assert_eq!(merged.len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }

    /// Full production shape: read-only cwd scan, creating data-root scan,
    /// merged result sorted by ID with cwd priority.
    #[test]
    fn merge_piper_roots_production_shape() {
        let cwd = unique_test_root("merge-prod-cwd");
        let data = unique_test_root("merge-prod-data");
        create_model_pair(&cwd, "zzz-cwd-model", 22050);
        create_model_pair(&cwd, "shared-model", 22050);
        create_invalid_json(&cwd, "broken-model");
        create_model_pair(&data, "shared-model", 22050);
        create_model_pair(&data, "aaa-data-model", 22050);

        let merged = merge_piper_roots(vec![
            scan_piper_models_dir(&cwd.join(MODELS_SUBDIR)),
            discover_piper_models(&data),
        ]);

        let ids: Vec<&str> = merged.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "local-piper:aaa-data-model",
                "local-piper:shared-model",
                "local-piper:zzz-cwd-model",
            ]
        );
        assert!(
            merged[1].onnx_path.starts_with(&cwd),
            "valid cwd model must win over the data-root duplicate"
        );
        assert!(
            merged[2].onnx_path.starts_with(&cwd),
            "cwd-only model stays available"
        );
        std::fs::remove_dir_all(&cwd).ok();
        std::fs::remove_dir_all(&data).ok();
    }
}
