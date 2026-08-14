// Prefix parsing module for TTS preprocessing
//
// Parses text prefixes that control event routing:
// - "!!text"    → skip both Twitch and WebView
// - "!text"     → skip Twitch, send to WebView
// - "!t" / "!t msg" → Twitch-only delivery (no TTS/WebView), boundary-safe token
// - "text"      → normal routing (both Twitch and WebView)

/// Result of parsing text prefixes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixResult {
    /// Text with prefix removed
    pub text: String,
    /// Skip sending to Twitch chat
    pub skip_twitch: bool,
    /// Skip sending to WebView
    pub skip_webview: bool,
    /// Route the phrase to Twitch only (no TTS pipeline).
    ///
    /// When `true`, `skip_twitch`/`skip_webview` carry no routing semantics:
    /// consumers must check `twitch_only` first.
    pub twitch_only: bool,
}

impl PrefixResult {
    /// Create a new PrefixResult
    pub fn new(text: String, skip_twitch: bool, skip_webview: bool) -> Self {
        Self {
            text,
            skip_twitch,
            skip_webview,
            twitch_only: false,
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

    /// Create a result with the Twitch-only `!t` prefix (skip TTS and WebView)
    pub fn twitch_only(text: String) -> Self {
        Self {
            text,
            skip_twitch: false,
            skip_webview: true,
            twitch_only: true,
        }
    }
}

/// Parse prefixes from text
///
/// # Examples
/// - `!!text` → PrefixResult { text: "text", skip_twitch: true, skip_webview: true }
/// - `!text`  → PrefixResult { text: "text", skip_twitch: true, skip_webview: false }
/// - `!t msg` → PrefixResult { text: "msg", twitch_only: true }
/// - `text`   → PrefixResult { text: "text", skip_twitch: false, skip_webview: false }
/// - ` !text` → PrefixResult { text: " !text", skip_twitch: false, skip_webview: false }
///   (leading space means no prefix)
///
/// `!t` is recognized as a standalone token only when followed by end-of-string
/// or whitespace. `!thanks`, `!t2go` and `!tвеличие` fall through to the legacy
/// single-`!` semantics (skip Twitch, text without `!`).
pub fn parse_prefix(text: &str) -> PrefixResult {
    if let Some(stripped) = text.strip_prefix("!!") {
        PrefixResult::skip_both(stripped.trim_start().to_string())
    } else if text.starts_with("!t") && is_twitch_only_boundary(text) {
        PrefixResult::twitch_only(text[2..].trim_start().to_string())
    } else if let Some(stripped) = text.strip_prefix('!') {
        PrefixResult::skip_twitch_only(stripped.trim_start().to_string())
    } else {
        PrefixResult::normal(text.to_string())
    }
}

/// `!t` is a standalone token only when followed by end-of-string or whitespace.
fn is_twitch_only_boundary(text: &str) -> bool {
    text[2..]
        .chars()
        .next()
        .map(|c| c.is_whitespace())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_double_bang() {
        let result = parse_prefix("!!Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert!(result.skip_twitch);
        assert!(result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_single_bang() {
        let result = parse_prefix("!Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert!(result.skip_twitch);
        assert!(!result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_no_prefix() {
        let result = parse_prefix("Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert!(!result.skip_twitch);
        assert!(!result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_leading_space_no_prefix() {
        let result = parse_prefix(" !Привет мир");
        assert_eq!(result.text, " !Привет мир");
        assert!(!result.skip_twitch);
        assert!(!result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_single_bang_trim() {
        let result = parse_prefix("!   Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert!(result.skip_twitch);
        assert!(!result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_double_bang_trim() {
        let result = parse_prefix("!!   Привет мир");
        assert_eq!(result.text, "Привет мир");
        assert!(result.skip_twitch);
        assert!(result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_empty_text() {
        let result = parse_prefix("!");
        assert_eq!(result.text, "");
        assert!(result.skip_twitch);
        assert!(!result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_empty_text_double() {
        let result = parse_prefix("!!");
        assert_eq!(result.text, "");
        assert!(result.skip_twitch);
        assert!(result.skip_webview);
        assert!(!result.twitch_only);
    }

    #[test]
    fn test_parse_twitch_only() {
        let result = parse_prefix("!t");
        assert_eq!(result.text, "");
        assert!(result.twitch_only);
        assert!(result.skip_webview);
        assert!(!result.skip_twitch);
    }

    #[test]
    fn test_parse_twitch_only_trailing_space() {
        let result = parse_prefix("!t ");
        assert_eq!(result.text, "");
        assert!(result.twitch_only);
        assert!(result.skip_webview);
        assert!(!result.skip_twitch);
    }

    #[test]
    fn test_parse_twitch_only_message() {
        let result = parse_prefix("!t msg");
        assert_eq!(result.text, "msg");
        assert!(result.twitch_only);
        assert!(result.skip_webview);
        assert!(!result.skip_twitch);
    }

    #[test]
    fn test_parse_twitch_only_message_trim() {
        let result = parse_prefix("!t   msg");
        assert_eq!(result.text, "msg");
        assert!(result.twitch_only);
        assert!(result.skip_webview);
        assert!(!result.skip_twitch);
    }

    #[test]
    fn test_parse_thanks_is_legacy_single_bang() {
        let result = parse_prefix("!thanks");
        assert_eq!(result.text, "thanks");
        assert!(!result.twitch_only);
        assert!(result.skip_twitch);
        assert!(!result.skip_webview);
    }

    #[test]
    fn test_parse_t2go_is_legacy_single_bang() {
        let result = parse_prefix("!t2go");
        assert_eq!(result.text, "t2go");
        assert!(!result.twitch_only);
        assert!(result.skip_twitch);
        assert!(!result.skip_webview);
    }

    #[test]
    fn test_parse_t_cyrillic_is_legacy_single_bang() {
        let result = parse_prefix("!tвеличие");
        assert_eq!(result.text, "tвеличие");
        assert!(!result.twitch_only);
        assert!(result.skip_twitch);
        assert!(!result.skip_webview);
    }
}
