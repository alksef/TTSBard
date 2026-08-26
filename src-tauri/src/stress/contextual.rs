//! Parsing and merging of `+`-marked stress notation into the neutral
//! structured-stress contract used by the TTS pipeline.
//!
//! The native RUAccent runtime and the user's manual Silero markers both use a
//! leading `+` before a stressed vowel. This module converts that notation into
//! provider-neutral [`StructuredStress`] byte-offset annotations and merges the
//! manual and automatic results without ever spawning a subprocess.

use crate::stress::annotations::{StressAnnotation, StructuredStress};
use std::collections::HashSet;

/// Convert accentor output into annotations over `original`.
///
/// The output must be the same text after removing every `+`, except that an
/// accentor may restore `е`/`ё`; a marker is only accepted immediately before a
/// Russian vowel. The `ё` choices are retained as the render base while the
/// original source text stays unchanged. This deliberately refuses every other
/// text rewrite instead of guessing offsets in user text.
pub fn structured_stress_from_marked(
    original: &str,
    marked: &str,
) -> Result<StructuredStress, String> {
    let mut plain = String::with_capacity(marked.len());
    let mut vowel_offsets = Vec::new();
    let mut chars = marked.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '+' {
            plain.push(ch);
            continue;
        }

        let vowel = chars
            .peek()
            .copied()
            .ok_or_else(|| "dangling stress marker".to_string())?;
        if !is_russian_vowel(vowel) {
            return Err("stress marker must precede a Russian vowel".to_string());
        }
        vowel_offsets.push(plain.len());
    }

    let annotations = vowel_offsets
        .into_iter()
        .map(|stressed_vowel| annotation_for_vowel(original, stressed_vowel))
        .collect::<Result<Vec<_>, _>>()?;
    StructuredStress::with_render_base(original.to_string(), plain, annotations)
}

/// Parse manually entered Silero `+` markers into the neutral stress contract.
///
/// A plus immediately before a Russian vowel is consumed as a stress marker;
/// any other plus remains literal text (for example, `C++`). The returned
/// original is therefore safe to pass to an accentor without exposing its
/// provider-specific syntax.
pub fn structured_stress_from_silero_marked(marked: &str) -> Result<StructuredStress, String> {
    let mut original = String::with_capacity(marked.len());
    let mut vowel_offsets = Vec::new();
    let mut chars = marked.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '+' && chars.peek().is_some_and(|vowel| is_russian_vowel(*vowel)) {
            vowel_offsets.push(original.len());
        } else {
            original.push(ch);
        }
    }

    let annotations = vowel_offsets
        .into_iter()
        .map(|stressed_vowel| annotation_for_vowel(&original, stressed_vowel))
        .collect::<Result<Vec<_>, _>>()?;
    StructuredStress::new(original, annotations)
}

/// Combine manual and automatic stress over the same clean text.
///
/// A manual marker owns its complete word, so an automatic marker for that
/// word is discarded. This makes manually corrected homographs stable across
/// repeated preview and synthesis passes. Automatic `ё` restoration is kept
/// only outside manually marked words: a manual marker owns the complete word,
/// including its `е`/`ё` spelling.
pub fn merge_manual_stress(
    manual: StructuredStress,
    automatic: StructuredStress,
) -> Result<StructuredStress, String> {
    if manual.original != automatic.original {
        return Err("manual and automatic stress use different original text".to_string());
    }

    let manual_words: HashSet<(usize, usize)> = manual
        .annotations
        .iter()
        .map(|annotation| (annotation.word_start, annotation.word_end))
        .collect();
    let mut render_base = automatic.render_base;
    for &(word_start, word_end) in &manual_words {
        render_base.replace_range(
            word_start..word_end,
            &manual.render_base[word_start..word_end],
        );
    }
    let mut annotations = manual.annotations;
    annotations.extend(automatic.annotations.into_iter().filter(|annotation| {
        !manual_words.contains(&(annotation.word_start, annotation.word_end))
    }));
    StructuredStress::with_render_base(manual.original, render_base, annotations)
}

fn annotation_for_vowel(text: &str, stressed_vowel: usize) -> Result<StressAnnotation, String> {
    if !text.is_char_boundary(stressed_vowel) {
        return Err("stress marker is not on a text boundary".to_string());
    }
    let vowel = text[stressed_vowel..]
        .chars()
        .next()
        .ok_or_else(|| "stress marker points outside text".to_string())?;
    if !is_russian_vowel(vowel) {
        return Err("stress marker must point to a Russian vowel".to_string());
    }

    let word_start = text[..stressed_vowel]
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_russian_letter(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let word_end = text[stressed_vowel..]
        .char_indices()
        .find(|(_, ch)| !is_russian_letter(*ch))
        .map(|(index, _)| stressed_vowel + index)
        .unwrap_or(text.len());

    if word_start == word_end {
        return Err("stress marker is outside a Russian word".to_string());
    }
    Ok(StressAnnotation {
        word_start,
        word_end,
        stressed_vowel,
    })
}

fn is_russian_letter(ch: char) -> bool {
    matches!(ch, 'А'..='Я' | 'а'..='я' | 'Ё' | 'ё')
}

fn is_russian_vowel(ch: char) -> bool {
    matches!(
        ch,
        'А' | 'а'
            | 'Е'
            | 'е'
            | 'Ё'
            | 'ё'
            | 'И'
            | 'и'
            | 'О'
            | 'о'
            | 'У'
            | 'у'
            | 'Ы'
            | 'ы'
            | 'Э'
            | 'э'
            | 'Ю'
            | 'ю'
            | 'Я'
            | 'я'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_one_marked_word() {
        let stress = structured_stress_from_marked("привет", "прив+ет").unwrap();
        assert_eq!(
            stress.annotations,
            vec![StressAnnotation {
                word_start: 0,
                word_end: 12,
                stressed_vowel: 8
            }]
        );
    }

    #[test]
    fn converts_two_words_and_uppercase() {
        let stress = structured_stress_from_marked("Привет, мир", "Прив+ет, м+ир").unwrap();
        assert_eq!(
            stress.annotations,
            vec![
                StressAnnotation {
                    word_start: 0,
                    word_end: 12,
                    stressed_vowel: 8
                },
                StressAnnotation {
                    word_start: 14,
                    word_end: 20,
                    stressed_vowel: 16
                },
            ]
        );
    }

    #[test]
    fn no_markers_means_empty_annotations() {
        assert!(structured_stress_from_marked("без маркера", "без маркера")
            .unwrap()
            .annotations
            .is_empty());
    }

    #[test]
    fn parses_manual_silero_markers_and_keeps_literal_plus() {
        let stress = structured_stress_from_silero_marked("C++ и з+амок").unwrap();
        assert_eq!(stress.original, "C++ и замок");
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&stress),
            "C++ и з+амок"
        );
    }

    #[test]
    fn manual_stress_wins_over_automatic_for_the_same_word() {
        let manual = structured_stress_from_silero_marked("замк+ом и м+ука").unwrap();
        let automatic = structured_stress_from_marked("замком и мука", "з+амком и мук+а").unwrap();
        let merged = merge_manual_stress(manual, automatic).unwrap();
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&merged),
            "замк+ом и м+ука"
        );
    }

    #[test]
    fn manual_parse_render_is_idempotent() {
        let marked = "з+амок под замк+ом";
        let once = structured_stress_from_silero_marked(marked).unwrap();
        let rendered = crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&once);
        let twice = structured_stress_from_silero_marked(&rendered).unwrap();
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&twice),
            marked
        );
    }

    #[test]
    fn rejects_changed_or_invalid_output() {
        assert!(structured_stress_from_marked("привет", "пока").is_err());
        assert!(structured_stress_from_marked("привет", "привет+").is_err());
        assert!(structured_stress_from_marked("привт", "прив+т").is_err());
    }

    #[test]
    fn accepts_accentor_yo_restoration_without_changing_original_text() {
        let stress = structured_stress_from_marked("все", "вс+ё").unwrap();
        assert_eq!(stress.original, "все");
        assert_eq!(stress.render_base, "всё");
        assert_eq!(
            stress.annotations,
            vec![StressAnnotation {
                word_start: 0,
                word_end: "все".len(),
                stressed_vowel: "вс".len(),
            }]
        );
    }

    #[test]
    fn silero_adapter_round_trips_marked_preview_text() {
        let original = "замок был на холме под замком";
        let marked = "зам+ок был на холме под замк+ом";
        let stress = structured_stress_from_marked(original, marked).unwrap();
        let rendered = crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&stress);
        assert_eq!(rendered, marked);
    }

    #[test]
    fn silero_adapter_round_trips_unmarked_text() {
        let original = "без омографов";
        let stress = structured_stress_from_marked(original, original).unwrap();
        let rendered = crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&stress);
        assert_eq!(rendered, original);
    }

    #[test]
    fn yo_restoration_reaches_silero_and_piper_rendering() {
        let manual = structured_stress_from_silero_marked("все").unwrap();
        let automatic = structured_stress_from_marked("все", "вс+ё").unwrap();
        let merged = merge_manual_stress(manual, automatic).unwrap();

        assert_eq!(merged.original, "все");
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&merged),
            "вс+ё"
        );
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::Piper.adapt(&merged),
            "всё\u{0301}"
        );
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::PlainText.adapt(&merged),
            "все"
        );
    }

    #[test]
    fn manual_word_overrides_automatic_yo_restoration() {
        let manual = structured_stress_from_silero_marked("вс+е ещё").unwrap();
        let automatic = structured_stress_from_marked("все ещё", "вс+ё ещ+ё").unwrap();
        let merged = merge_manual_stress(manual, automatic).unwrap();

        assert_eq!(merged.render_base, "все ещё");
        assert_eq!(
            crate::stress::adapters::ProviderStressAdapter::Silero.adapt(&merged),
            "вс+е ещ+ё"
        );
    }
}
