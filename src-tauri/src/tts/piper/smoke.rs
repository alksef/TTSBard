#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    fn find_espeak_ng_data() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("PIPER_ESPEAKNG_DATA_DIRECTORY") {
            let p = PathBuf::from(&dir);
            if p.join("voices").exists() && p.join("en_dict").exists() {
                return Some(p);
            }
        }

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let resource_path = manifest_dir.join("resources").join("espeak-ng-data");
        if resource_path.join("voices").exists() && resource_path.join("en_dict").exists() {
            return Some(resource_path);
        }

        if let Ok(dir) = std::env::var("TTSBARD_STAGING_DIR") {
            let p = PathBuf::from(&dir).join("espeak-ng-data");
            if p.join("voices").exists() && p.join("en_dict").exists() {
                return Some(p);
            }
        }

        None
    }

    #[test]
    fn espeak_ng_data_present() {
        let data_dir = find_espeak_ng_data().expect(
            "espeak-ng-data not found. \
             Set PIPER_ESPEAKNG_DATA_DIRECTORY, TTSBARD_STAGING_DIR, \
             or run from project root.",
        );

        let voices_dir = data_dir.join("voices");
        assert!(
            voices_dir.is_dir(),
            "voices/ directory missing in {}",
            voices_dir.display()
        );

        let en_dict = data_dir.join("en_dict");
        assert!(
            en_dict.is_file(),
            "en_dict missing in {}",
            en_dict.display()
        );

        let file_count = std::fs::read_dir(&data_dir)
            .expect("failed to read espeak-ng-data directory")
            .count();
        assert!(
            file_count > 10,
            "espeak-ng-data has only {} entries; expected > 10 (lang, voices, dictionaries)",
            file_count
        );

        eprintln!(
            "espeak-ng-data found at {} with {} top-level entries",
            data_dir.display(),
            file_count
        );
    }

    #[test]
    fn espeak_ng_data_has_required_voice_files() {
        let data_dir = find_espeak_ng_data().expect("espeak-ng-data not found");

        let lang_dir = data_dir.join("lang");
        assert!(
            lang_dir.is_dir(),
            "lang/ directory missing — phoneme tables unavailable"
        );

        let voices_dir = data_dir.join("voices");
        let voice_variants: Vec<_> = std::fs::read_dir(&voices_dir)
            .expect("cannot read voices/")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|sub| std::fs::read_dir(sub.path()).ok())
            .flat_map(|entries| entries.filter_map(|e| e.ok()))
            .filter(|e| e.path().is_file())
            .collect();
        assert!(
            !voice_variants.is_empty(),
            "voices/ has no voice variant files"
        );
    }
}
