use crate::secret_log;
use reqwest::Client;
use std::time::Duration;
use tracing::{error, info};

/// Parse proxy URL and create appropriate reqwest::Proxy.
///
/// Supports schemes: socks5, socks5h, socks4, socks4a, http, https.
pub fn parse_proxy_url(url: &str) -> Result<reqwest::Proxy, String> {
    let (scheme, _rest) = url
        .split_once("://")
        .ok_or_else(|| "Invalid proxy URL: missing scheme".to_string())?;

    let scheme_lower = scheme.to_lowercase();
    if !matches!(
        scheme_lower.as_str(),
        "socks5" | "socks5h" | "socks4" | "socks4a" | "http" | "https"
    ) {
        return Err(format!("Unsupported proxy URL scheme: {}", scheme));
    }

    reqwest::Proxy::all(url)
        .map_err(|e| {
            error!(error = %e, proxy_scheme = %scheme, safe_url = %secret_log::safe_url_for_log(url), "Failed to create proxy");
            format!("Failed to create {} proxy: {}", scheme, e)
        })
}

/// Build a reqwest::Client with optional proxy and timeout.
pub fn build_client_with_proxy(
    proxy_url: Option<&str>,
    timeout: Duration,
) -> Result<Client, String> {
    if let Some(proxy_url) = proxy_url {
        let proxy = parse_proxy_url(proxy_url)?;
        info!(has_proxy = true, safe_url = %secret_log::safe_url_for_log(proxy_url), "Using proxy");
        Client::builder()
            .proxy(proxy)
            .timeout(timeout)
            .build()
            .map_err(|e| {
                error!(error = %e, "Failed to build client with proxy");
                format!("Failed to build client with proxy: {}", e)
            })
    } else {
        info!("Direct connection (no proxy)");
        Client::builder().timeout(timeout).build().map_err(|e| {
            error!(error = %e, "Failed to build client");
            format!("Failed to build client: {}", e)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ---------- parse_proxy_url ----------

    #[test]
    fn parse_accepted_schemes() {
        let cases = [
            "socks5://127.0.0.1:1080",
            "socks5h://127.0.0.1:1080",
            "socks4://127.0.0.1:1080",
            "socks4a://127.0.0.1:1080",
            "http://127.0.0.1:8080",
            "https://127.0.0.1:8443",
            "socks5://user:pass@127.0.0.1:1080",
            "http://[::1]:8080",
            "http://[2001:db8::1]:8080",
            "socks5://proxy.example.com:1080",
        ];
        for url in &cases {
            assert!(parse_proxy_url(url).is_ok(), "expected Ok for: {}", url);
        }
    }

    #[test]
    fn parse_scheme_case_insensitive() {
        let cases = [
            "SOCKs5://127.0.0.1:1080",
            "SOCks5H://127.0.0.1:1080",
            "SOCKS4://127.0.0.1:1080",
            "SOCKS4A://127.0.0.1:1080",
            "HTTP://127.0.0.1:8080",
            "HTTPS://127.0.0.1:8443",
        ];
        for url in &cases {
            assert!(parse_proxy_url(url).is_ok(), "expected Ok for: {}", url);
        }
    }

    #[test]
    fn parse_missing_scheme_fails() {
        assert!(parse_proxy_url("127.0.0.1:1080").is_err());
    }

    #[test]
    fn parse_unsupported_scheme_fails() {
        assert!(parse_proxy_url("ftp://127.0.0.1:21").is_err());
    }

    #[test]
    fn parse_malformed_url_fails() {
        assert!(parse_proxy_url("socks5://a b:1080").is_err());
    }

    // ---------- build_client_with_proxy ----------

    #[test]
    fn build_client_direct_mode_succeeds() {
        let result = build_client_with_proxy(None, Duration::from_secs(30));
        assert!(result.is_ok(), "direct mode should succeed");
    }

    #[test]
    fn build_client_valid_proxy_succeeds() {
        let result =
            build_client_with_proxy(Some("socks5://127.0.0.1:1080"), Duration::from_secs(30));
        assert!(result.is_ok(), "valid proxy should succeed");
    }

    #[test]
    fn build_client_invalid_proxy_fails() {
        let result = build_client_with_proxy(Some("ftp://127.0.0.1:21"), Duration::from_secs(30));
        assert!(result.is_err(), "invalid proxy scheme should fail");
    }
}
