// Prefix parsing module for TTS preprocessing
//
// Parses text prefixes that control event routing:
// - "!!text" → skip both Twitch and WebView
// - "!text"  → skip Twitch, send to WebView
// - "text"   → normal routing (both Twitch and WebView)

/// Result of parsing text prefixes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixResult {
    /// Text with prefix removed
    pub text: String,
    /// Skip sending to Twitch chat
    pub skip_twitch: bool,
    /// Skip sending to WebView
    pub skip_webview: bool,
}

impl PrefixResult {
    /// Create a new PrefixResult
    pub fn new(text: String, skip_twitch: bool, skip_webview: bool) -> Self {
        Self {
            text,
            skip_twitch,
            skip_webview,
        }
    }

    /// Create a result with no prefix (normal routing)
    pub fn normal(text: String) -> Self {
        Self::new(text, false, false)
    }

    /// Create a result with single ! prefix (skip Twitch only)
    pub fn skip_twitch_only(text: String) -> Self {
        Self::new(text, true, false)
    }

    /// Create a result with !! prefix (skip both)
    pub fn skip_both(text: String) -> Self {
        Self::new(text, true, true)
    }
}

/// Parse prefixes from text
///
/// # Examples
/// - `!!text` → PrefixResult { text: "text", skip_twitch: true, skip_webview: true }
/// - `!text`  → PrefixResult { text: "text", skip_twitch: true, skip_webview: false }
/// - `text`   → PrefixResult { text: "text", skip_twitch: false, skip_webview: false }
/// - ` !text` → PrefixResult { text: " !text", skip_twitch: false, skip_webview: false }
///            (leading space means no prefix)
pub fn parse_prefix(text: &str) -> PrefixResult {
    if text.starts_with("!!") {
        PrefixResult::skip_both(text[2..].trim_start().to_string())
    } else if text.starts_with('!') {
        PrefixResult::skip_twitch_only(text[1..].trim_start().to_string())
    } else {
        PrefixResult::normal(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_double_bang() {
        let result = parse_prefix("!!Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert_eq!(result.skip_twitch, true);
        assert_eq!(result.skip_webview, true);
    }

    #[test]
    fn test_parse_single_bang() {
        let result = parse_prefix("!Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert_eq!(result.skip_twitch, true);
        assert_eq!(result.skip_webview, false);
    }

    #[test]
    fn test_parse_no_prefix() {
        let result = parse_prefix("Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert_eq!(result.skip_twitch, false);
        assert_eq!(result.skip_webview, false);
    }

    #[test]
    fn test_parse_leading_space_no_prefix() {
        let result = parse_prefix(" !Привет мир");
        assert_eq!(result.text, " !Привет мир");
        assert_eq!(result.skip_twitch, false);
        assert_eq!(result.skip_webview, false);
    }

    #[test]
    fn test_parse_single_bang_trim() {
        let result = parse_prefix("!   Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert_eq!(result.skip_twitch, true);
        assert_eq!(result.skip_webview, false);
    }

    #[test]
    fn test_parse_double_bang_trim() {
        let result = parse_prefix("!!   Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert_eq!(result.skip_twitch, true);
        assert_eq!(result.skip_webview, true);
    }

    #[test]
    fn test_parse_empty_text() {
        let result = parse_prefix("!");
        assert_eq!(result.text, "");
        assert_eq!(result.skip_twitch, true);
        assert_eq!(result.skip_webview, false);
    }

    #[test]
    fn test_parse_empty_text_double() {
        let result = parse_prefix("!!");
        assert_eq!(result.text, "");
        assert_eq!(result.skip_twitch, true);
        assert_eq!(result.skip_webview, true);
    }
}
