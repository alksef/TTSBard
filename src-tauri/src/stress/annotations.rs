/// A single stressed-word annotation on the original text.
///
/// All offsets are UTF-8 byte offsets into the original string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressAnnotation {
    /// First byte of the word.
    pub word_start: usize,
    /// One past the last byte of the word (exclusive).
    pub word_end: usize,
    /// First byte of the stressed vowel inside the word.
    pub stressed_vowel: usize,
}

/// Original text together with validated stress annotations and a corrected
/// render base.
#[derive(Debug, Clone)]
pub struct StructuredStress {
    /// The original unstressed source text, unchanged by automatic synthesis.
    pub original: String,
    /// Text used for rendering, retaining only validated RUAccent `е ↔ ё`
    /// substitutions relative to `original`.
    pub render_base: String,
    /// Validated stress annotations over the original text.
    pub annotations: Vec<StressAnnotation>,
}

impl StructuredStress {
    /// Validate and store stress annotations with an identical render base.
    pub fn new(original: String, annotations: Vec<StressAnnotation>) -> Result<Self, String> {
        Self::with_render_base(original.clone(), original, annotations)
    }

    /// Validate and store stress annotations together with a corrected render
    /// base.
    ///
    /// The render base must preserve the byte length and character count of the
    /// original text and may only differ by same-case `е`/`ё` or `Е`/`Ё`
    /// substitutions.
    pub fn with_render_base(
        original: String,
        render_base: String,
        annotations: Vec<StressAnnotation>,
    ) -> Result<Self, String> {
        validate_annotations(&original, &annotations)?;
        validate_render_base(&original, &render_base)?;
        Ok(Self {
            original,
            render_base,
            annotations,
        })
    }
}

/// Validate that annotations are on UTF-8 char boundaries, their word ranges
/// are non-empty and inside the original text, their stressed vowel lies inside
/// the word, and word ranges never overlap or duplicate.
fn validate_annotations(original: &str, annotations: &[StressAnnotation]) -> Result<(), String> {
    let len = original.len();

    for annotation in annotations {
        let start = annotation.word_start;
        let end = annotation.word_end;
        let vowel = annotation.stressed_vowel;

        if !original.is_char_boundary(start) {
            return Err(format!(
                "word_start {start} is not on a UTF-8 char boundary"
            ));
        }
        if !original.is_char_boundary(end) {
            return Err(format!("word_end {end} is not on a UTF-8 char boundary"));
        }
        if !original.is_char_boundary(vowel) {
            return Err(format!(
                "stressed_vowel {vowel} is not on a UTF-8 char boundary"
            ));
        }

        if start >= end || end > len {
            return Err(format!(
                "word range [{start}, {end}) is empty or outside the original text"
            ));
        }

        if vowel < start || vowel >= end {
            return Err(format!(
                "stressed_vowel {vowel} is outside word range [{start}, {end})"
            ));
        }
    }

    for i in 0..annotations.len() {
        for j in (i + 1)..annotations.len() {
            let a = &annotations[i];
            let b = &annotations[j];
            let overlaps = a.word_start < b.word_end && b.word_start < a.word_end;
            if overlaps {
                return Err(format!(
                    "word ranges [{}, {}) and [{}, {}) overlap or duplicate",
                    a.word_start, a.word_end, b.word_start, b.word_end
                ));
            }
        }
    }

    Ok(())
}

/// Validate a render base against its original text.
///
/// The render base must keep the same byte length and character count as the
/// original, and every difference must be a same-case `е`/`ё` or `Е`/`Ё`
/// substitution. Both characters are two UTF-8 bytes, so preserving the
/// character count also preserves all byte offsets used by the neutral stress
/// contract.
fn validate_render_base(original: &str, render_base: &str) -> Result<(), String> {
    if original.len() != render_base.len() {
        return Err("render base byte length differs from original text".to_string());
    }
    if original.chars().count() != render_base.chars().count() {
        return Err("render base character count differs from original text".to_string());
    }

    let only_yo = original
        .chars()
        .zip(render_base.chars())
        .all(|(left, right)| {
            left == right
                || matches!(
                    (left, right),
                    ('е' | 'ё', 'е' | 'ё') | ('Е' | 'Ё', 'Е' | 'Ё')
                )
        });
    if !only_yo {
        return Err("render base changes original text".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ann(word_start: usize, word_end: usize, stressed_vowel: usize) -> StressAnnotation {
        StressAnnotation {
            word_start,
            word_end,
            stressed_vowel,
        }
    }

    #[test]
    fn accepts_valid_cyrillic_byte_offsets() {
        let original = "привет мир".to_string();
        // "привет" -> bytes [0, 12), stressed 'е' at byte 8.
        // "мир" -> bytes [13, 19), stressed 'и' at byte 15.
        let annotations = vec![ann(0, 12, 8), ann(13, 19, 15)];

        let stress = StructuredStress::new(original.clone(), annotations.clone()).unwrap();
        assert_eq!(stress.original, original);
        assert_eq!(stress.annotations, annotations);
    }

    #[test]
    fn rejects_offsets_not_on_char_boundary() {
        let original = "привет".to_string();
        assert!(StructuredStress::new(original.clone(), vec![ann(1, 12, 8)]).is_err());
        assert!(StructuredStress::new(original.clone(), vec![ann(0, 12, 1)]).is_err());
        assert!(StructuredStress::new(original.clone(), vec![ann(0, 11, 8)]).is_err());
    }

    #[test]
    fn rejects_empty_and_outside_ranges() {
        let original = "привет".to_string();
        assert!(StructuredStress::new(original.clone(), vec![ann(2, 2, 2)]).is_err());
        assert!(StructuredStress::new(original.clone(), vec![ann(0, 13, 8)]).is_err());
    }

    #[test]
    fn rejects_vowel_outside_word() {
        let original = "привет".to_string();
        assert!(StructuredStress::new(original.clone(), vec![ann(0, 6, 8)]).is_err());
        assert!(StructuredStress::new(original.clone(), vec![ann(2, 12, 0)]).is_err());
    }

    #[test]
    fn rejects_overlapping_and_duplicate_word_ranges() {
        let original = "привет мир".to_string();
        assert!(
            StructuredStress::new(original.clone(), vec![ann(0, 12, 8), ann(6, 19, 8)]).is_err()
        );
        assert!(
            StructuredStress::new(original.clone(), vec![ann(0, 12, 8), ann(0, 12, 8)]).is_err()
        );
    }

    #[test]
    fn new_sets_render_base_identical_to_original() {
        let stress = StructuredStress::new("привет".to_string(), vec![ann(0, 12, 8)]).unwrap();
        assert_eq!(stress.render_base, "привет");
    }

    #[test]
    fn with_render_base_accepts_yo_restoration() {
        let stress = StructuredStress::with_render_base(
            "все".to_string(),
            "всё".to_string(),
            vec![ann(0, "все".len(), "вс".len())],
        )
        .unwrap();
        assert_eq!(stress.original, "все");
        assert_eq!(stress.render_base, "всё");
    }

    #[test]
    fn with_render_base_accepts_uppercase_yo() {
        let stress = StructuredStress::with_render_base(
            "ВСЕ".to_string(),
            "ВСЁ".to_string(),
            vec![ann(0, "ВСЕ".len(), "ВС".len())],
        )
        .unwrap();
        assert_eq!(stress.render_base, "ВСЁ");
    }

    #[test]
    fn with_render_base_rejects_length_change() {
        assert!(StructuredStress::with_render_base(
            "привет".to_string(),
            "пока".to_string(),
            vec![],
        )
        .is_err());
    }

    #[test]
    fn with_render_base_rejects_other_changes() {
        assert!(StructuredStress::with_render_base(
            "замок".to_string(),
            "замак".to_string(),
            vec![],
        )
        .is_err());
    }
}
