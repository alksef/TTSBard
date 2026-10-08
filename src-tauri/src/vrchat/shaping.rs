use unicode_segmentation::UnicodeSegmentation;

/// Maximum number of Unicode scalar values allowed in a VRChat chatbox message.
pub const MAX_CHATBOX_CHARS: usize = 144;

/// Trailing ellipsis character appended when text exceeds `MAX_CHATBOX_CHARS`.
pub const ELLIPSIS_CHAR: char = '…';

/// Prepares a text copy suitable for sending to VRChat chatbox.
///
/// Rules:
/// - If `raw_text` is empty or contains only whitespace, returns `None` (no message sent).
/// - If `raw_text` has <= 144 Unicode scalar values, returns `Some(raw_text.to_string())`.
/// - If `raw_text` exceeds 144 Unicode scalar values, truncates to fit within 144 scalar
///   values with a trailing `…` (U+2026). Truncation never splits an extended grapheme cluster.
/// - Does not modify the input string. Explicit newlines are preserved as-is.
pub fn prepare_chatbox_text(raw_text: &str) -> Option<String> {
    if raw_text.trim().is_empty() {
        return None;
    }

    let char_count = raw_text.chars().count();
    if char_count <= MAX_CHATBOX_CHARS {
        return Some(raw_text.to_string());
    }

    let max_prefix_chars = MAX_CHATBOX_CHARS - 1; // 143 scalar values reserved for prefix
    let mut accumulated_chars = 0;
    let mut byte_offset = 0;

    for (offset, grapheme) in raw_text.grapheme_indices(true) {
        let grapheme_chars = grapheme.chars().count();
        if accumulated_chars + grapheme_chars > max_prefix_chars {
            break;
        }
        accumulated_chars += grapheme_chars;
        byte_offset = offset + grapheme.len();
    }

    let mut truncated = String::with_capacity(byte_offset + ELLIPSIS_CHAR.len_utf8());
    truncated.push_str(&raw_text[..byte_offset]);
    truncated.push(ELLIPSIS_CHAR);
    Some(truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_and_whitespace_only() {
        assert_eq!(prepare_chatbox_text(""), None);
        assert_eq!(prepare_chatbox_text("   "), None);
        assert_eq!(prepare_chatbox_text("\t\n\r  "), None);
    }

    #[test]
    fn test_short_ascii_and_cyrillic() {
        assert_eq!(prepare_chatbox_text("Hello"), Some("Hello".to_string()));
        let ru = "Привет, мир!";
        assert_eq!(prepare_chatbox_text(ru), Some(ru.to_string()));
    }

    #[test]
    fn test_preserves_explicit_newlines() {
        let text_with_nl = "Line 1\nLine 2\r\nLine 3";
        assert_eq!(
            prepare_chatbox_text(text_with_nl),
            Some(text_with_nl.to_string())
        );
    }

    #[test]
    fn test_boundaries_143_144_145_ascii() {
        let s143: String = "a".repeat(143);
        assert_eq!(prepare_chatbox_text(&s143), Some(s143.clone()));
        assert_eq!(s143.chars().count(), 143);

        let s144: String = "b".repeat(144);
        assert_eq!(prepare_chatbox_text(&s144), Some(s144.clone()));
        assert_eq!(s144.chars().count(), 144);

        let s145: String = "c".repeat(145);
        let shaped145 = prepare_chatbox_text(&s145).expect("should produce truncated text");
        assert_eq!(shaped145.chars().count(), 144);
        assert!(shaped145.ends_with(ELLIPSIS_CHAR));
        let expected145 = format!("{}…", "c".repeat(143));
        assert_eq!(shaped145, expected145);
    }

    #[test]
    fn test_boundaries_143_144_145_cyrillic() {
        // 'ж' is a 2-byte UTF-8 char, 1 Unicode scalar value
        let ru143: String = "ж".repeat(143);
        assert_eq!(prepare_chatbox_text(&ru143), Some(ru143.clone()));

        let ru144: String = "ж".repeat(144);
        assert_eq!(prepare_chatbox_text(&ru144), Some(ru144.clone()));

        let ru145: String = "ж".repeat(145);
        let shaped = prepare_chatbox_text(&ru145).expect("should produce truncated text");
        assert_eq!(shaped.chars().count(), 144);
        assert!(shaped.ends_with('…'));
        let expected = format!("{}…", "ж".repeat(143));
        assert_eq!(shaped, expected);
    }

    #[test]
    fn test_multi_scalar_emoji_no_split() {
        // Flag of Russia "🇷🇺": 2 regional indicator scalar values: U+1F1F7, U+1F1FA.
        // It's 1 grapheme cluster, but 2 chars.
        let flag = "🇷🇺";
        assert_eq!(flag.chars().count(), 2);

        // Case A: 141 chars + flag (2 chars) = 143 chars. Total <= 144 without overflow, but let's test with trailing excess.
        // 141 'a' + flag (2 chars) + "extra":
        // 141 chars + 2 chars = 143 chars.
        // Adding next grapheme ("e") would exceed 143 chars.
        // So prefix should include 141 'a's and the entire flag, followed by '…'.
        let text_a = format!("{}a{}extra", "a".repeat(140), flag); // 140 + 1 + 2 + 5 = 148 chars
        let shaped_a = prepare_chatbox_text(&text_a).unwrap();
        let expected_a = format!("{}a{}…", "a".repeat(140), flag);
        assert_eq!(shaped_a, expected_a);
        assert_eq!(shaped_a.chars().count(), 144);

        // Case B: 142 chars + flag (2 chars) + "extra":
        // 142 'a' + flag (2 chars) = 144 chars > 143 reserved max prefix!
        // The flag cannot fit in 143 chars.
        // Because we NEVER split the flag, the flag must NOT be included partially!
        // The prefix must stop at 142 'a's, followed by '…'.
        // Total chars: 142 + 1 = 143 chars (<= 144).
        let text_b = format!("{}{}extra", "a".repeat(142), flag);
        let shaped_b = prepare_chatbox_text(&text_b).unwrap();
        let expected_b = format!("{}…", "a".repeat(142));
        assert_eq!(shaped_b, expected_b);
        assert_eq!(shaped_b.chars().count(), 143);

        // Case C: Complex emoji with ZWJ: "👨‍👩‍👧‍👦"
        // (U+1F468 U+200D U+1F469 U+200D U+1F467 U+200D U+1F466) -> 7 scalar values in 1 grapheme.
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(family.chars().count(), 7);

        // 136 'x' + family (7 chars) = 143 chars.
        let text_c = format!("{}{}{}more", "x".repeat(136), family, "z");
        let shaped_c = prepare_chatbox_text(&text_c).unwrap();
        let expected_c = format!("{}{}…", "x".repeat(136), family);
        assert_eq!(shaped_c, expected_c);
        assert_eq!(shaped_c.chars().count(), 144);

        // 137 'x' + family (7 chars) = 144 chars > 143.
        // family cannot fit without splitting. Thus it is excluded.
        let text_d = format!("{}{}{}more", "x".repeat(137), family, "z");
        let shaped_d = prepare_chatbox_text(&text_d).unwrap();
        let expected_d = format!("{}…", "x".repeat(137));
        assert_eq!(shaped_d, expected_d);
        assert_eq!(shaped_d.chars().count(), 138);
    }
}
