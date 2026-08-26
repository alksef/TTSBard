use crate::stress::annotations::StructuredStress;

/// Stress-marker encoding for a specific TTS provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStressAdapter {
    /// Silero: `+` inserted before each stressed vowel.
    Silero,
    /// Plain text: stress annotations are ignored, original is returned.
    PlainText,
    /// Piper/eSpeak: U+0301 follows each stressed vowel.
    Piper,
}

impl ProviderStressAdapter {
    /// Render the stressed text for the provider.
    pub fn adapt(&self, stress: &StructuredStress) -> String {
        match self {
            ProviderStressAdapter::Silero => silero_adapt(stress),
            ProviderStressAdapter::Piper => piper_adapt(stress),
            ProviderStressAdapter::PlainText => stress.original.clone(),
        }
    }
}

fn silero_adapt(stress: &StructuredStress) -> String {
    let mut result = stress.render_base.clone();
    let mut vowels: Vec<usize> = stress
        .annotations
        .iter()
        .map(|annotation| annotation.stressed_vowel)
        .collect();
    vowels.sort_unstable();
    for vowel in vowels.into_iter().rev() {
        result.insert(vowel, '+');
    }
    result
}

fn piper_adapt(stress: &StructuredStress) -> String {
    let mut result = stress.render_base.clone();
    let mut insertion_points: Vec<usize> = stress
        .annotations
        .iter()
        .map(|annotation| {
            annotation.stressed_vowel
                + stress.render_base[annotation.stressed_vowel..]
                    .chars()
                    .next()
                    .expect("validated stressed vowel must point at a character")
                    .len_utf8()
        })
        .collect();
    insertion_points.sort_unstable();
    for point in insertion_points.into_iter().rev() {
        result.insert(point, '\u{0301}');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stress::annotations::StressAnnotation;

    fn word_offsets(text: &str, word: &str) -> (usize, usize) {
        let start = text.find(word).expect("word must occur in text");
        (start, start + word.len())
    }

    fn vowel_offset(text: &str, word: &str, vowel: char) -> usize {
        let (start, _) = word_offsets(text, word);
        start + word.find(vowel).expect("vowel must occur in word")
    }

    fn two_annotations() -> (StructuredStress, String) {
        let original = "красивая девушка".to_string();
        let (w1_start, w1_end) = word_offsets(&original, "красивая");
        let (w2_start, w2_end) = word_offsets(&original, "девушка");
        let annotations = vec![
            StressAnnotation {
                word_start: w1_start,
                word_end: w1_end,
                stressed_vowel: vowel_offset(&original, "красивая", 'и'),
            },
            StressAnnotation {
                word_start: w2_start,
                word_end: w2_end,
                stressed_vowel: vowel_offset(&original, "девушка", 'е'),
            },
        ];
        let expected = "крас+ивая д+евушка".to_string();
        (
            StructuredStress::new(original, annotations).unwrap(),
            expected,
        )
    }

    #[test]
    fn silero_inserts_plus_before_stressed_vowels() {
        let (stress, expected) = two_annotations();
        assert_eq!(ProviderStressAdapter::Silero.adapt(&stress), expected);
    }

    #[test]
    fn piper_inserts_combining_acute_after_stressed_cyrillic_vowels() {
        let (stress, _) = two_annotations();
        let rendered = ProviderStressAdapter::Piper.adapt(&stress);
        assert_eq!(rendered, "краси́вая де́вушка");
        assert!(!rendered.contains('+'));
    }

    #[test]
    fn passthrough_adapters_return_original() {
        let (stress, _) = two_annotations();
        let original = stress.original.clone();
        assert_eq!(ProviderStressAdapter::PlainText.adapt(&stress), original);
    }

    #[test]
    fn silero_renders_restored_yo_plus_marker() {
        let stress = StructuredStress::with_render_base(
            "все".to_string(),
            "всё".to_string(),
            vec![StressAnnotation {
                word_start: 0,
                word_end: "все".len(),
                stressed_vowel: "вс".len(),
            }],
        )
        .unwrap();
        assert_eq!(ProviderStressAdapter::Silero.adapt(&stress), "вс+ё");
        assert_eq!(ProviderStressAdapter::PlainText.adapt(&stress), "все");
    }

    #[test]
    fn piper_renders_restored_yo_with_combining_stress() {
        let stress = StructuredStress::with_render_base(
            "все".to_string(),
            "всё".to_string(),
            vec![StressAnnotation {
                word_start: 0,
                word_end: "все".len(),
                stressed_vowel: "вс".len(),
            }],
        )
        .unwrap();
        let rendered = ProviderStressAdapter::Piper.adapt(&stress);
        assert_eq!(rendered, "всё\u{0301}");
        assert!(!rendered.contains('+'));
    }
}
