use reqwest::Client;
use std::time::Duration;
use tracing::{info, error};

/// Parse proxy URL and create appropriate reqwest::Proxy.
///
/// Supports schemes: socks5, socks5h, socks4, socks4a, http, https.
pub fn parse_proxy_url(url: &str) -> Result<reqwest::Proxy, String> {
    let (scheme, _rest) = url.split_once("://")
        .ok_or_else(|| "Invalid proxy URL: missing scheme".to_string())?;

    let scheme_lower = scheme.to_lowercase();
    if !matches!(scheme_lower.as_str(), "socks5" | "socks5h" | "socks4" | "socks4a" | "http" | "https") {
        return Err(format!("Unsupported proxy URL scheme: {}", scheme));
    }

    reqwest::Proxy::all(url)
        .map_err(|e| {
            error!(error = %e, proxy_url = %url, scheme = %scheme, "Failed to create proxy");
            format!("Failed to create {} proxy: {}", scheme, e)
        })
}

/// Build a reqwest::Client with optional proxy and timeout.
pub fn build_client_with_proxy(proxy_url: Option<&str>, timeout: Duration) -> Result<Client, String> {
    if let Some(proxy_url) = proxy_url {
        let proxy = parse_proxy_url(proxy_url)?;
        info!(proxy_url = %proxy_url, "Using proxy");
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
        Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| {
                error!(error = %e, "Failed to build client");
                format!("Failed to build client: {}", e)
            })
    }
}
