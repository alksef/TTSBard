/// Safe-logging helpers for masking secrets and normalizing URLs.
///
/// Policy:
/// - Secrets are NEVER logged in plain text.
/// - Diagnostic metadata (provider, model, has_value, status, lengths) is preserved.
/// - URLs are normalized: scheme://host:port only (no user:pass, query, fragment).
/// - Absolute filesystem paths are replaced with safe markers.
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "[MASK]".to_string();
    }
    let char_count = value.chars().count();
    if char_count < 9 {
        return "[MASK]".to_string();
    }
    let first_4: String = value.chars().take(4).collect();
    let last_4: String = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}[MASK]{}", first_4, last_4)
}

pub fn safe_url_for_log(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        let after_auth = match rest.find('@') {
            Some(at_idx) => &rest[at_idx + 1..],
            None => rest,
        };
        let end = after_auth
            .find('/')
            .into_iter()
            .chain(after_auth.find('?'))
            .chain(after_auth.find('#'))
            .min();
        let host_port = match end {
            Some(idx) => &after_auth[..idx],
            None => after_auth,
        };
        format!("{}://{}", scheme, host_port)
    } else {
        "[invalid-url]".to_string()
    }
}

/// Replace absolute path roots with safe markers.
///
/// Known roots:
/// - `%APPDATA%` → `[APP_DATA]`
/// - `%LOCALAPPDATA%` → `[LOCAL_APP_DATA]`
/// - `%TEMP%` / `%TMP%` → `[TEMP]`
/// - `%USERPROFILE%` → `[USER_PROFILE]`
/// - current working directory → `[WORKDIR]`
///
/// Relative paths are returned as-is.
/// Unknown absolute paths return `[ABSOLUTE_PATH]` — no partial leak.
/// Root matching is case-insensitive on Windows.
pub fn safe_path_for_log(path: &Path) -> String {
    if path.is_relative() {
        return path.display().to_string();
    }

    let roots = known_roots();

    if let Some((marker, root)) = roots
        .iter()
        .find(|(_, root)| starts_with_case_insensitive(path, root))
    {
        let relative = path
            .components()
            .skip(root.components().count())
            .collect::<PathBuf>();
        if relative.as_os_str().is_empty() {
            return marker.clone();
        }
        return format!("{}/{}", marker, relative.display());
    }

    "[ABSOLUTE_PATH]".to_string()
}

fn known_roots() -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();

    if let Ok(p) = std::env::var("TEMP").or_else(|_| std::env::var("TMP")) {
        roots.push(("[TEMP]".to_string(), PathBuf::from(p)));
    }
    if let Ok(p) = std::env::var("APPDATA") {
        roots.push(("[APP_DATA]".to_string(), PathBuf::from(p)));
    }
    if let Ok(p) = std::env::var("LOCALAPPDATA") {
        roots.push(("[LOCAL_APP_DATA]".to_string(), PathBuf::from(p)));
    }
    if let Ok(p) = std::env::var("USERPROFILE") {
        roots.push(("[USER_PROFILE]".to_string(), PathBuf::from(p)));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(("[WORKDIR]".to_string(), cwd));
    }

    // Sort longest-path-first so more specific roots match before broader ones
    roots.sort_by_key(|b| std::cmp::Reverse(b.1.as_os_str().len()));

    roots
}

fn starts_with_case_insensitive(path: &Path, root: &Path) -> bool {
    let mut path_components = path.components();
    let mut root_components = root.components();

    loop {
        match (root_components.next(), path_components.next()) {
            (None, _) => return true,
            (Some(_), None) => return false,
            (Some(rc), Some(pc)) => {
                if !component_eq(rc, pc) {
                    return false;
                }
            }
        }
    }
}

fn component_eq(a: std::path::Component<'_>, b: std::path::Component<'_>) -> bool {
    use std::path::Component;
    match (a, b) {
        (Component::Normal(a), Component::Normal(b)) => a.eq_ignore_ascii_case(b),
        (a, b) => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- mask_secret ----------

    #[test]
    fn mask_normal_key() {
        let result = mask_secret("sk-abc123def456ghi789jkl");
        assert_eq!(result, "sk-a[MASK]9jkl");
    }

    #[test]
    fn mask_exactly_nine_chars() {
        let result = mask_secret("123456789");
        assert_eq!(result, "1234[MASK]6789");
    }

    #[test]
    fn mask_exactly_eight_chars_full_mask() {
        let result = mask_secret("12345678");
        assert_eq!(result, "[MASK]");
    }

    #[test]
    fn mask_short_value() {
        let result = mask_secret("abc");
        assert_eq!(result, "[MASK]");
    }

    #[test]
    fn mask_empty_value() {
        let result = mask_secret("");
        assert_eq!(result, "[MASK]");
    }

    #[test]
    fn mask_unicode_value() {
        let result = mask_secret("секрет-длинный-ключ");
        let first_4_chars: String = result.chars().take(4).collect();
        assert_eq!(first_4_chars, "секр");
        assert!(result.contains("[MASK]"));
        assert_eq!(result.chars().count(), 4 + 6 + 4); // 4 + [MASK] (6) + 4 = 14
    }

    #[test]
    fn mask_value_never_returns_original() {
        let cases = ["", "a", "ab", "abc123", "short", "longer-test-key-example"];
        for case in &cases {
            let masked = mask_secret(case);
            assert_ne!(
                masked, *case,
                "mask_secret returned original for {:?}",
                case
            );
            assert!(!masked.is_empty());
        }
    }

    #[test]
    fn mask_is_stable() {
        let a = mask_secret("my-secret-key-12345");
        let b = mask_secret("my-secret-key-12345");
        assert_eq!(a, b);
    }

    // ---------- safe_url_for_log ----------

    #[test]
    fn url_with_user_pass_stripped() {
        let safe = safe_url_for_log("socks5://user:pass@host.com:1080");
        assert_eq!(safe, "socks5://host.com:1080");
    }

    #[test]
    fn url_with_query_stripped() {
        let safe = safe_url_for_log("http://example.com/api?token=abc123&key=xyz");
        assert_eq!(safe, "http://example.com");
    }

    #[test]
    fn url_with_fragment_stripped() {
        let safe = safe_url_for_log("https://example.com/page#secret");
        assert_eq!(safe, "https://example.com");
    }

    #[test]
    fn url_with_user_pass_query_and_fragment_stripped() {
        let safe = safe_url_for_log("https://user:pass@host:443/path?token=x#frag");
        assert_eq!(safe, "https://host:443");
    }

    #[test]
    fn url_path_stripped() {
        let safe = safe_url_for_log("https://api.openai.com/v1");
        assert_eq!(safe, "https://api.openai.com");
    }

    #[test]
    fn url_with_token_like_path() {
        let safe = safe_url_for_log("https://api.example.com/v1/chat?token=abc123");
        assert_eq!(safe, "https://api.example.com");
    }

    #[test]
    fn url_with_credentials_in_query_and_fragment() {
        let safe = safe_url_for_log("https://example.com/page?key=secret&token=abc#frag");
        assert_eq!(safe, "https://example.com");
    }

    #[test]
    fn url_ipv6_with_port_and_path() {
        let safe = safe_url_for_log("https://[::1]:8080/path?query=val");
        assert_eq!(safe, "https://[::1]:8080");
    }

    #[test]
    fn url_with_port_unchanged() {
        let safe = safe_url_for_log("http://127.0.0.1:8124");
        assert_eq!(safe, "http://127.0.0.1:8124");
    }

    #[test]
    fn invalid_url_returns_placeholder() {
        let safe = safe_url_for_log("not-a-url");
        assert_eq!(safe, "[invalid-url]");
    }

    #[test]
    fn empty_url_returns_placeholder() {
        let safe = safe_url_for_log("");
        assert_eq!(safe, "[invalid-url]");
    }

    #[test]
    fn query_only_url_strips_query() {
        let safe = safe_url_for_log("https://host?token=secret");
        assert_eq!(safe, "https://host");
    }

    #[test]
    fn fragment_only_url_strips_fragment() {
        let safe = safe_url_for_log("https://host#section");
        assert_eq!(safe, "https://host");
    }

    #[test]
    fn query_before_path_strips_at_query() {
        let safe = safe_url_for_log("https://host?token=secret/path");
        assert_eq!(safe, "https://host");
    }

    // ---------- safe_path_for_log ----------

    #[test]
    fn relative_path_unchanged() {
        let safe = safe_path_for_log(Path::new("config/settings.json"));
        assert_eq!(safe, "config/settings.json");
    }

    #[test]
    fn appdata_with_tail() {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_else(|_| "C:\\Users\\test\\AppData\\Roaming".to_string());
        let p = Path::new(&appdata).join("ttsbard").join("telegram.session");
        let safe = safe_path_for_log(&p);
        assert!(safe.starts_with("[APP_DATA]"));
        assert!(
            safe.ends_with("ttsbard/telegram.session")
                || safe.ends_with("ttsbard\\telegram.session")
        );
        assert!(!safe.contains(&appdata));
    }

    #[test]
    fn userprofile_path_is_safe() {
        let userprofile =
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\test".to_string());
        let p = Path::new(&userprofile).join("Documents").join("notes.txt");
        let safe = safe_path_for_log(&p);
        assert!(safe.starts_with("[USER_PROFILE]"));
        assert!(!safe.contains(&userprofile));
    }

    #[test]
    fn unknown_absolute_path_returns_marker() {
        let safe = safe_path_for_log(Path::new("C:\\Some\\Unknown\\Dir\\file.txt"));
        assert_eq!(safe, "[ABSOLUTE_PATH]");
    }

    #[test]
    fn temp_path_is_safe() {
        let tmp = match std::env::var("TEMP").or_else(|_| std::env::var("TMP")) {
            Ok(v) => v,
            Err(_) => return, // TEMP not available
        };
        let p = Path::new(&tmp).join("some_file.log");
        let safe = safe_path_for_log(&p);
        assert!(safe.starts_with("[TEMP]"));
        assert!(!safe.contains(&tmp));
    }

    #[test]
    fn case_insensitive_root_matching() {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Test\\AppData\\Roaming".to_string());
        // Uppercase the drive letter to test case insensitivity
        let mangled = {
            let mut chars: Vec<char> = appdata.chars().collect();
            if chars.len() > 1 && chars[0].is_ascii_alphabetic() && chars[1] == ':' {
                chars[0] = chars[0].to_ascii_uppercase();
            }
            let s: String = chars.into_iter().collect();
            Path::new(&s).join("ttsbard").join("settings.json")
        };
        let safe = safe_path_for_log(&mangled);
        assert!(safe.starts_with("[APP_DATA]"));
    }

    #[test]
    fn localappdata_path_is_safe() {
        let local = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\test\\AppData\\Local".to_string());
        let p = Path::new(&local).join("Packages").join("sample.dat");
        let safe = safe_path_for_log(&p);
        assert!(safe.starts_with("[LOCAL_APP_DATA]"));
        assert!(!safe.contains(&local));
    }

    #[test]
    fn workdir_path_is_safe() {
        let cwd = std::env::current_dir().expect("current_dir");
        let p = cwd.join("Cargo.toml");
        let safe = safe_path_for_log(&p);
        assert!(safe.starts_with("[WORKDIR]"));
        assert!(!safe.contains(&*cwd.to_string_lossy()));
    }
}
