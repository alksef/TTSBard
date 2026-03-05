#[cfg(test)]
mod tests {
    // Test helper function that mimics the filter logic
    fn is_allowed_character(ch: char) -> bool {
        ch.is_ascii_graphic() || (!ch.is_ascii() && !ch.is_control())
    }

    #[test]
    fn test_shift_numbers_are_allowed() {
        // Shift+0-9 symbols
        let shift_numbers = ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'];
        for &ch in &shift_numbers {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_punctuation_is_allowed() {
        // Common punctuation
        let punctuation = ['.', ',', ';', ':', '?', '!', '\'', '"'];
        for &ch in &punctuation {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_special_symbols_are_allowed() {
        // Special symbols including backslash, pipe, etc.
        let special = ['\\', '|', '/', '@', '#', '$', '%', '^', '&', '*', '_', '+', '-', '=', '~', '`'];
        for &ch in &special {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_brackets_are_allowed() {
        let brackets = ['(', ')', '[', ']', '{', '}', '<', '>'];
        for &ch in &brackets {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_unicode_is_allowed() {
        // Cyrillic and other non-ASCII should be allowed
        let unicode = ['А', 'Б', 'В', 'а', 'б', 'в', '€', '©'];
        for &ch in &unicode {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_control_characters_are_blocked() {
        // Control characters should be blocked
        let control = ['\x00', '\x01', '\x02', '\x07', '\x08', '\x1B', '\x7F'];
        for &ch in &control {
            assert!(!is_allowed_character(ch), "Control character '{:x}' should be blocked", ch as u32);
        }
    }
}
