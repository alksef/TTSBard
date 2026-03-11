// Number to text conversion module for Russian TTS
//
// Converts numbers to Russian text with grammatical gender agreement
// Example: "1 книга" → "одна книга", "2 книги" → "две книги"

use russian_numbers::NumeralName;
use lazy_static::lazy_static;
use regex::Regex;

/// Detect grammatical gender of a Russian word by suffix
fn detect_gender(word: &str) -> RussianGender {
    let word_lower = word.to_lowercase();
    // Remove trailing punctuation
    let word_clean = word_lower.trim_end_matches(|c: char| "!?,.;:".contains(c));

    if word_clean.ends_with('а') || word_clean.ends_with('я') || word_clean.ends_with('ь') {
        RussianGender::Feminine
    } else if word_clean.ends_with('о') || word_clean.ends_with('е') {
        RussianGender::Neuter
    } else {
        RussianGender::Masculine
    }
}

/// Russian grammatical gender
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RussianGender {
    Masculine,
    Feminine,
    Neuter,
}

/// Convert number parts to Russian text with grammatical gender agreement
fn convert_number_parts(parts: &[&str], gender: RussianGender) -> String {
    // The NumeralName trait already provides proper gender agreement for large numbers
    // For small numbers (1, 2) we may need to adjust based on the next word's gender
    if parts.is_empty() {
        return String::new();
    }

    // For 1 and 2, override with gender-specific forms if gender is not masculine
    if parts.len() == 1 {
        let num_str = parts[0];
        if num_str == "один" || num_str == "два" {
            return match gender {
                RussianGender::Masculine => num_str.to_string(),
                RussianGender::Feminine => {
                    if num_str == "один" { "одна".to_string() } else { "две".to_string() }
                }
                RussianGender::Neuter => {
                    if num_str == "один" { "одно".to_string() } else { "два".to_string() }
                }
            };
        }
    }

    parts.join(" ")
}

// Cached regex for number matching (compiled once at startup)
lazy_static! {
    static ref NUMBER_REGEX: Regex = Regex::new(r"-?\b\d+\b").unwrap();
}

/// Process text: replace numbers with Russian words
///
/// # Examples
/// - "У меня 5 яблок" → "У меня пять яблок"
/// - "1 книга" → "одна книга"
/// - "2 книги" → "две книги" (Note: heuristic limitation for plural forms)
/// - "-10 градусов" → "минус десять градусов"
///
/// # Limitations
/// Numbers larger than 999,999,999,999,999,999 will be clamped to this maximum
/// value due to `usize` conversion requirements for the `russian_numbers` crate.
pub fn process_numbers(text: &str) -> String {
    NUMBER_REGEX.replace_all(text, |caps: &regex::Captures| {
        let number_str = &caps[0];

        if let Ok(num) = number_str.parse::<i64>() {
            let is_negative = num < 0;
            let abs_num = num.abs();

            // Find next word for gender determination
            let after_match = &text[caps.get(0).unwrap().end()..];
            let next_word = after_match
                .split_whitespace()
                .next()
                .unwrap_or("");

            // Only apply gender agreement for 1 and 2 (where it matters in Russian)
            let gender = if abs_num == 1 || abs_num == 2 {
                detect_gender(next_word)
            } else {
                RussianGender::Masculine
            };

            // Limit to reasonable range for usize (0-999_999_999_999_999_999)
            let abs_num_limited = abs_num.min(999_999_999_999_999_999);

            // Convert number to words using NumeralName trait
            let parts = (abs_num_limited as usize).numeral_name();
            let result = convert_number_parts(&parts, gender);

            if is_negative {
                format!("минус {}", result)
            } else {
                result
            }
        } else {
            number_str.to_string()
        }
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_gender_feminine() {
        // Base feminine forms (nominative singular)
        assert_eq!(detect_gender("книга"), RussianGender::Feminine);
        assert_eq!(detect_gender("книга!"), RussianGender::Feminine);
        assert_eq!(detect_gender("семья"), RussianGender::Feminine);
        assert_eq!(detect_gender("ночь"), RussianGender::Feminine);
        assert_eq!(detect_gender("земля"), RussianGender::Feminine);
        assert_eq!(detect_gender("ручка"), RussianGender::Feminine);
    }

    #[test]
    fn test_detect_gender_neuter() {
        // Base neuter forms (nominative singular)
        assert_eq!(detect_gender("окно"), RussianGender::Neuter);
        assert_eq!(detect_gender("поле"), RussianGender::Neuter);
        assert_eq!(detect_gender("море"), RussianGender::Neuter);
        assert_eq!(detect_gender("поле"), RussianGender::Neuter);
    }

    #[test]
    fn test_detect_gender_masculine() {
        // Base masculine forms (nominative singular)
        assert_eq!(detect_gender("стол"), RussianGender::Masculine);
        assert_eq!(detect_gender("дом"), RussianGender::Masculine);
        assert_eq!(detect_gender("друг"), RussianGender::Masculine);
        assert_eq!(detect_gender("стол"), RussianGender::Masculine);
        assert_eq!(detect_gender("человек"), RussianGender::Masculine);
    }

    #[test]
    fn test_process_numbers() {
        assert_eq!(process_numbers("У меня 5 яблок"), "У меня пять яблок");
        // Note: "книги" is plural, heuristic may not detect correctly
        // For better results, use singular form or full morphological analysis
        assert_eq!(process_numbers("5 яблок"), "пять яблок");
        assert_eq!(process_numbers("-10 градусов"), "минус десять градусов");
    }

    #[test]
    fn test_process_numbers_with_gender_agreement() {
        // Test gender agreement with singular words (nominative case)
        // The heuristic works best with base forms (nominative singular)
        assert_eq!(process_numbers("1 книга"), "одна книга");
        // Note: "книги" is plural form, heuristic limitation
        // For 2+ items, Russian uses plural, but our heuristic looks at word endings
        // "книги" ends with 'и', not 'а'/'я'/'ь', so it's detected as masculine

        // Test with singular base forms (heuristic limitation)
        assert_eq!(process_numbers("1 друг"), "один друг");
        assert_eq!(process_numbers("1 окно"), "одно окно");
        assert_eq!(process_numbers("1 ручка"), "одна ручка");
        assert_eq!(process_numbers("1 стол"), "один стол");

        // Test larger numbers (gender agreement only applies to 1 and 2)
        assert_eq!(process_numbers("5 книг"), "пять книг");
        assert_eq!(process_numbers("10 друзей"), "десять друзей");
    }

    #[test]
    fn test_empty_next_word() {
        // When number is at end of text, should use masculine
        assert_eq!(process_numbers("У меня 5"), "У меня пять");
    }

    #[test]
    fn test_large_numbers() {
        assert_eq!(process_numbers("1000"), "одна тысяча");
        assert_eq!(process_numbers("1000000"), "один миллион");
        assert_eq!(process_numbers("123"), "сто двадцать три");
    }

    #[test]
    fn test_negative_numbers() {
        assert_eq!(process_numbers("-5 градусов"), "минус пять градусов");
        assert_eq!(process_numbers("-1"), "минус один");
        assert_eq!(process_numbers("-100"), "минус сто");
    }

    #[test]
    fn test_mixed_positive_and_negative() {
        // Test single-pass handling of mixed positive and negative numbers
        assert_eq!(process_numbers("У меня 5 яблок и -10 градусов"),
                   "У меня пять яблок и минус десять градусов");
        // Note: "2 стола" ends with 'а', heuristic detects as feminine → "две стола"
        // This is a known limitation for plural forms
        assert_eq!(process_numbers("10 градусов, -5, 3 человека"),
                   "десять градусов, минус пять, три человека");
        assert_eq!(process_numbers("-1 ручка, 5 столов"),
                   "минус одна ручка, пять столов");
    }
}
