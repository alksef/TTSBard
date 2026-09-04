//! OCR model-pack manifest contract and safe one-level scanner.
//!
//! Backend-only P0 slice (ROADMAP-088): no ONNX session, settings, commands or
//! UI is wired up yet, so most items are consumed only by tests until a runtime
//! introduces callers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Supported OCR model families.
///
/// Only `pp_ocr_v5` is supported in the MVP. Unknown families are rejected
/// during manifest deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum OcrFamily {
    #[serde(rename = "pp_ocr_v5")]
    PpOcrV5,
}

/// Typed contract for an OCR model pack `manifest.json`.
///
/// Filenames are relative to the pack directory and must be single relative
/// file names (no absolute paths, no separators, no `.`/`..` components).
#[derive(Debug, Clone, Deserialize)]
pub struct OcrPackManifest {
    pub id: String,
    pub display_name: String,
    pub languages: Vec<String>,
    pub family: OcrFamily,
    pub det_file: String,
    pub rec_file: String,
    pub dict_file: String,
}

/// Reason a candidate OCR pack was rejected during scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    InvalidDirectoryName,
    ManifestNotRegularFile,
    ManifestReadFailed,
    ManifestParseFailed,
    InvalidId,
    BlankDisplayName,
    NoLanguages,
    BlankLanguageTag,
    DuplicateLanguageTag,
    InvalidFileName,
    DuplicateFileName,
    DirectoryNameMismatch,
    MissingModelFile,
}

/// Descriptor for a validated OCR model pack.
///
/// Produced purely from filesystem/metadata discovery: no ONNX session is
/// created and no model files are loaded. The descriptor carries just enough
/// stable identity and resolved path information to build a runtime later.
#[derive(Debug, Clone)]
pub struct OcrPackDescriptor {
    pub id: String,
    pub display_name: String,
    pub languages: Vec<String>,
    pub family: OcrFamily,
    pub pack_root: PathBuf,
    pub det_path: PathBuf,
    pub rec_path: PathBuf,
    pub dict_path: PathBuf,
}

const OCR_MODELS_SUBDIR: &str = "models/ocr";
const MANIFEST_FILE: &str = "manifest.json";

impl OcrPackManifest {
    pub fn validate(&self) -> Result<(), ScanError> {
        if !is_valid_id(&self.id) {
            return Err(ScanError::InvalidId);
        }
        if self.display_name.trim().is_empty() {
            return Err(ScanError::BlankDisplayName);
        }
        if self.languages.is_empty() {
            return Err(ScanError::NoLanguages);
        }

        let mut seen: Vec<String> = Vec::new();
        for lang in &self.languages {
            let tag = lang.trim();
            if tag.is_empty() {
                return Err(ScanError::BlankLanguageTag);
            }
            let key = tag.to_ascii_lowercase();
            if seen.contains(&key) {
                return Err(ScanError::DuplicateLanguageTag);
            }
            seen.push(key);
        }

        for name in [&self.det_file, &self.rec_file, &self.dict_file] {
            if !is_single_relative_file_name(name) {
                return Err(ScanError::InvalidFileName);
            }
        }

        if !names_distinct_case_insensitive(&self.det_file, &self.rec_file, &self.dict_file) {
            return Err(ScanError::DuplicateFileName);
        }

        Ok(())
    }
}

/// Scan the immediate children of `<app_data_root>/models/ocr` for valid packs.
///
/// Returns only complete, valid packs. A missing/unreadable root yields an
/// empty list and is never created by this pure scanner. A broken pack is
/// skipped independently without hiding later valid packs. Results are sorted
/// by `id`.
pub fn scan_ocr_packs(app_data_root: &Path) -> Vec<OcrPackDescriptor> {
    let ocr_root = app_data_root.join(OCR_MODELS_SUBDIR);

    let entries = match std::fs::read_dir(&ocr_root) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(
                dir = %crate::secret_log::safe_path_for_log(&ocr_root),
                error = %e,
                "Failed to read OCR models directory"
            );
            return Vec::new();
        }
    };

    let mut results = Vec::new();

    for entry in entries.flatten() {
        let pack_dir = entry.path();

        if !is_plain_directory(&pack_dir) {
            continue;
        }

        match read_pack(&pack_dir) {
            Ok(descriptor) => {
                tracing::info!(id = %descriptor.id, "Discovered OCR model pack");
                results.push(descriptor);
            }
            Err(reason) => {
                tracing::warn!(
                    pack = %crate::secret_log::safe_path_for_log(&pack_dir),
                    reason = ?reason,
                    "Skipping OCR model pack"
                );
            }
        }
    }

    results.sort_by(|a, b| a.id.cmp(&b.id));

    if !results.is_empty() {
        tracing::info!(count = results.len(), "OCR model pack discovery complete");
    }

    results
}

fn read_pack(pack_dir: &Path) -> Result<OcrPackDescriptor, ScanError> {
    let dir_name = pack_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ScanError::InvalidDirectoryName)?;

    let manifest_path = pack_dir.join(MANIFEST_FILE);
    if !is_plain_regular_file(&manifest_path) {
        return Err(ScanError::ManifestNotRegularFile);
    }

    let content =
        std::fs::read_to_string(&manifest_path).map_err(|_| ScanError::ManifestReadFailed)?;

    let manifest: OcrPackManifest =
        serde_json::from_str(&content).map_err(|_| ScanError::ManifestParseFailed)?;

    manifest.validate()?;

    if dir_name != manifest.id {
        return Err(ScanError::DirectoryNameMismatch);
    }

    let det_path = pack_dir.join(&manifest.det_file);
    let rec_path = pack_dir.join(&manifest.rec_file);
    let dict_path = pack_dir.join(&manifest.dict_file);

    for path in [&det_path, &rec_path, &dict_path] {
        if !is_plain_nonempty_regular_file(path) {
            return Err(ScanError::MissingModelFile);
        }
    }

    Ok(OcrPackDescriptor {
        id: manifest.id,
        display_name: manifest.display_name,
        languages: manifest.languages,
        family: manifest.family,
        pack_root: pack_dir.to_path_buf(),
        det_path,
        rec_path,
        dict_path,
    })
}

/// Validate a portable stable pack `id`: non-empty ASCII lowercase letters,
/// digits and single hyphens only; no leading/trailing or repeated hyphens.
fn is_valid_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let bytes = id.as_bytes();
    if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' {
        return false;
    }
    let mut prev_hyphen = false;
    for &b in bytes {
        match b {
            b'a'..=b'z' | b'0'..=b'9' => prev_hyphen = false,
            b'-' => {
                if prev_hyphen {
                    return false;
                }
                prev_hyphen = true;
            }
            _ => return false,
        }
    }
    true
}

/// A single relative file name: not absolute, not `.`/`..`, no separators.
fn is_single_relative_file_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name == "." || name == ".." {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains(':') {
        return false;
    }
    true
}

fn names_distinct_case_insensitive(a: &str, b: &str, c: &str) -> bool {
    let al = a.to_ascii_lowercase();
    let bl = b.to_ascii_lowercase();
    let cl = c.to_ascii_lowercase();
    al != bl && al != cl && bl != cl
}

fn is_reparse_point(file_type: &std::fs::FileType) -> bool {
    if file_type.is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        file_type.is_symlink_dir() || file_type.is_symlink_file()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn is_plain_directory(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            let ft = meta.file_type();
            !is_reparse_point(&ft) && ft.is_dir()
        }
        Err(_) => false,
    }
}

fn is_plain_regular_file(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            let ft = meta.file_type();
            !is_reparse_point(&ft) && ft.is_file()
        }
        Err(_) => false,
    }
}

fn is_plain_nonempty_regular_file(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            let ft = meta.file_type();
            !is_reparse_point(&ft) && ft.is_file() && meta.len() > 0
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_root(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ttsbard-ocr-test-{}-{}-{}",
            std::process::id(),
            unique,
            name
        ))
    }

    fn ocr_root(root: &Path) -> PathBuf {
        root.join(OCR_MODELS_SUBDIR)
    }

    fn default_manifest(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "display_name": "eSlav PP-OCRv5 Mobile",
            "languages": ["ru", "en"],
            "family": "pp_ocr_v5",
            "det_file": "det.onnx",
            "rec_file": "rec.onnx",
            "dict_file": "dict.txt"
        })
    }

    fn write_manifest(dir: &Path, manifest: &serde_json::Value) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(MANIFEST_FILE), manifest.to_string()).unwrap();
    }

    fn write_model_file(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"dummy model content").unwrap();
    }

    fn write_valid_pack(root: &Path, id: &str) -> PathBuf {
        let dir = ocr_root(root).join(id);
        write_manifest(&dir, &default_manifest(id));
        write_model_file(&dir, "det.onnx");
        write_model_file(&dir, "rec.onnx");
        write_model_file(&dir, "dict.txt");
        dir
    }

    fn manifest(id: &str) -> OcrPackManifest {
        OcrPackManifest {
            id: id.to_string(),
            display_name: "Name".to_string(),
            languages: vec!["ru".to_string()],
            family: OcrFamily::PpOcrV5,
            det_file: "det.onnx".to_string(),
            rec_file: "rec.onnx".to_string(),
            dict_file: "dict.txt".to_string(),
        }
    }

    #[test]
    fn valid_packs_are_discovered_and_sorted() {
        let root = unique_test_root("valid-sorted");
        write_valid_pack(&root, "zzz-pack");
        write_valid_pack(&root, "aaa-pack");
        write_valid_pack(&root, "mmm-pack");

        let packs = scan_ocr_packs(&root);
        let ids: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["aaa-pack", "mmm-pack", "zzz-pack"]);

        let aaa = &packs[0];
        assert_eq!(aaa.display_name, "eSlav PP-OCRv5 Mobile");
        assert_eq!(aaa.languages, vec!["ru".to_string(), "en".to_string()]);
        assert_eq!(aaa.family, OcrFamily::PpOcrV5);
        assert_eq!(aaa.pack_root, ocr_root(&root).join("aaa-pack"));
        assert_eq!(
            aaa.det_path,
            ocr_root(&root).join("aaa-pack").join("det.onnx")
        );
        assert_eq!(
            aaa.rec_path,
            ocr_root(&root).join("aaa-pack").join("rec.onnx")
        );
        assert_eq!(
            aaa.dict_path,
            ocr_root(&root).join("aaa-pack").join("dict.txt")
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_root_returns_empty_without_creating() {
        let root = unique_test_root("missing-root");
        assert!(!root.exists());

        let packs = scan_ocr_packs(&root);
        assert!(packs.is_empty());
        assert!(!root.exists(), "scanner must not create directories");
    }

    #[test]
    fn empty_root_returns_empty() {
        let root = unique_test_root("empty-root");
        std::fs::create_dir_all(ocr_root(&root)).unwrap();

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn malformed_manifest_skipped_independently() {
        let root = unique_test_root("malformed");
        write_valid_pack(&root, "good-pack");

        let bad = ocr_root(&root).join("bad-pack");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join(MANIFEST_FILE), b"not valid json {{{").unwrap();

        let packs = scan_ocr_packs(&root);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].id, "good-pack");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn manifest_missing_fields_skipped() {
        let root = unique_test_root("missing-fields");
        let bad = ocr_root(&root).join("bad-pack");
        write_manifest(
            &bad,
            &serde_json::json!({
                "id": "bad-pack",
                "display_name": "x"
            }),
        );

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_model_file_skipped() {
        let root = unique_test_root("missing-file");
        let dir = ocr_root(&root).join("pack-a");
        write_manifest(&dir, &default_manifest("pack-a"));
        write_model_file(&dir, "det.onnx");
        write_model_file(&dir, "dict.txt");

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn empty_model_file_skipped() {
        let root = unique_test_root("empty-file");
        let dir = ocr_root(&root).join("pack-a");
        write_manifest(&dir, &default_manifest("pack-a"));
        write_model_file(&dir, "det.onnx");
        write_model_file(&dir, "rec.onnx");
        std::fs::write(dir.join("dict.txt"), b"").unwrap();

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn broken_first_pack_does_not_hide_valid_pack() {
        let root = unique_test_root("broken-first");
        let broken = ocr_root(&root).join("aaa-broken");
        write_manifest(&broken, &serde_json::json!({"not": "a manifest"}));
        write_valid_pack(&root, "zzz-good");

        let packs = scan_ocr_packs(&root);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].id, "zzz-good");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn directory_id_mismatch_skipped() {
        let root = unique_test_root("id-mismatch");
        let dir = ocr_root(&root).join("dir-name");
        write_manifest(&dir, &default_manifest("other-id"));
        write_model_file(&dir, "det.onnx");
        write_model_file(&dir, "rec.onnx");
        write_model_file(&dir, "dict.txt");

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn invalid_id_forms_rejected() {
        for id in [
            "", "-abc", "abc-", "a--b", "ABC", "ab_c", "a b", "a.b", "a-b-",
        ] {
            assert!(!is_valid_id(id), "id {:?} should be invalid", id);
        }
        for id in ["a", "ab", "a-b", "abc-123", "pp-ocr-v5-mobile"] {
            assert!(is_valid_id(id), "id {:?} should be valid", id);
        }
    }

    #[test]
    fn invalid_id_in_manifest_rejected() {
        let mut m = manifest("ABC-");
        m.id = "ABC".to_string();
        assert_eq!(m.validate(), Err(ScanError::InvalidId));
    }

    #[test]
    fn blank_display_name_rejected() {
        let mut m = manifest("p");
        m.display_name = "   ".to_string();
        assert_eq!(m.validate(), Err(ScanError::BlankDisplayName));
    }

    #[test]
    fn no_languages_rejected() {
        let mut m = manifest("p");
        m.languages = vec![];
        assert_eq!(m.validate(), Err(ScanError::NoLanguages));
    }

    #[test]
    fn blank_language_rejected() {
        let mut m = manifest("p");
        m.languages = vec!["  ".to_string()];
        assert_eq!(m.validate(), Err(ScanError::BlankLanguageTag));
    }

    #[test]
    fn duplicate_languages_rejected() {
        let mut m = manifest("p");
        m.languages = vec!["en".to_string(), "EN".to_string()];
        assert_eq!(m.validate(), Err(ScanError::DuplicateLanguageTag));
    }

    #[test]
    fn unsupported_family_rejected() {
        let root = unique_test_root("family");
        let dir = ocr_root(&root).join("pack-a");
        let mut m = default_manifest("pack-a");
        m["family"] = serde_json::json!("unknown_family");
        write_manifest(&dir, &m);
        write_model_file(&dir, "det.onnx");
        write_model_file(&dir, "rec.onnx");
        write_model_file(&dir, "dict.txt");

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unsafe_filenames_rejected() {
        for name in [
            "",
            ".",
            "..",
            "../x",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "C:\\x\\det.onnx",
            "C:det.onnx",
        ] {
            assert!(
                !is_single_relative_file_name(name),
                "name {:?} should be invalid",
                name
            );
        }
        for name in ["det.onnx", "rec.onnx", "dict.txt", "a-b.onnx"] {
            assert!(
                is_single_relative_file_name(name),
                "name {:?} should be valid",
                name
            );
        }
    }

    #[test]
    fn duplicate_filenames_rejected() {
        let mut m = manifest("p");
        m.rec_file = "det.onnx".to_string();
        assert_eq!(m.validate(), Err(ScanError::DuplicateFileName));
    }

    #[test]
    fn case_insensitive_duplicate_filenames_rejected() {
        let mut m = manifest("p");
        m.det_file = "Det.Onnx".to_string();
        m.rec_file = "det.onnx".to_string();
        assert_eq!(m.validate(), Err(ScanError::DuplicateFileName));
    }

    #[test]
    fn traversal_filename_rejected() {
        let mut m = manifest("p");
        m.dict_file = "../outside.txt".to_string();
        assert_eq!(m.validate(), Err(ScanError::InvalidFileName));
    }

    #[test]
    fn nested_packs_not_discovered() {
        let root = unique_test_root("nested");
        let outer = write_valid_pack(&root, "outer");

        let nested = outer.join("inner");
        write_manifest(&nested, &default_manifest("inner"));
        write_model_file(&nested, "det.onnx");
        write_model_file(&nested, "rec.onnx");
        write_model_file(&nested, "dict.txt");

        let packs = scan_ocr_packs(&root);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].id, "outer");

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_pack_directory_rejected() {
        use std::os::unix::fs::symlink;

        let root = unique_test_root("symlink-dir");
        std::fs::create_dir_all(ocr_root(&root)).unwrap();

        let real = unique_test_root("real-pack");
        let real_pack = write_valid_pack(&real, "pack-a");

        let link = ocr_root(&root).join("pack-a");
        symlink(&real_pack, &link).unwrap();

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&real).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_model_file_rejected() {
        use std::os::unix::fs::symlink;

        let root = unique_test_root("symlink-file");
        let dir = ocr_root(&root).join("pack-a");
        write_manifest(&dir, &default_manifest("pack-a"));
        write_model_file(&dir, "rec.onnx");
        write_model_file(&dir, "dict.txt");

        let external = root.join("external.onnx");
        std::fs::write(&external, b"data").unwrap();
        symlink(&external, dir.join("det.onnx")).unwrap();

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(windows)]
    #[test]
    fn junction_pack_directory_rejected() {
        let root = unique_test_root("junction-dir");
        std::fs::create_dir_all(ocr_root(&root)).unwrap();

        let real = unique_test_root("real-pack");
        let real_pack = write_valid_pack(&real, "pack-a");

        let link = ocr_root(&root).join("pack-a");

        let link_arg = link.to_string_lossy().replace('/', "\\");
        let target_arg = real_pack.to_string_lossy().replace('/', "\\");

        let output = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(&link_arg)
            .arg(&target_arg)
            .output()
            .unwrap_or_else(|e| panic!("failed to launch cmd for mklink: {e}"));

        assert!(
            output.status.success(),
            "mklink /J failed with exit {}. stdout: {} stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            std::fs::symlink_metadata(&link).is_ok(),
            "junction was not created at {}",
            link.display()
        );

        assert!(scan_ocr_packs(&root).is_empty());

        std::fs::remove_dir(&link).ok();
        std::fs::remove_dir_all(&real).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(windows)]
    #[test]
    fn symlinked_model_file_rejected_windows() {
        use std::os::windows::fs::symlink_file;

        let root = unique_test_root("symlink-file-win");
        let dir = ocr_root(&root).join("pack-a");
        write_manifest(&dir, &default_manifest("pack-a"));
        write_model_file(&dir, "rec.onnx");
        write_model_file(&dir, "dict.txt");

        let external = root.join("external.onnx");
        std::fs::write(&external, b"data").unwrap();

        if symlink_file(&external, dir.join("det.onnx")).is_ok() {
            assert!(scan_ocr_packs(&root).is_empty());
        } else {
            // Symlink privilege unavailable; skip assertion.
        }

        std::fs::remove_dir_all(&root).ok();
    }
}
