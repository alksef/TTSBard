use serde::{Deserialize, Serialize};
use tauri::{command, State};
use tracing::{info, error, warn};
use std::mem;
use crate::config::{SettingsManager, dto::ProxySettingsDto, ProxyType, ProxyMode};
use crate::commands::telegram::TelegramState;
use crate::telegram::ProxyStatus;

/// Result of testing a proxy connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResultDto {
    /// Whether the connection was successful
    pub success: bool,
    /// Connection latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Proxy type that was tested
    pub mode: String,
    /// Error message if connection failed
    pub error: Option<String>,
}

/// Test proxy connection to Telegram
///
/// This command tests a proxy connection by making a request to https://telegram.org
/// through the specified proxy server. It supports HTTP, SOCKS4, and SOCKS5 proxies.
///
/// # Arguments
/// * `proxy_type` - Type of proxy (http, socks4, socks5)
/// * `host` - Proxy server hostname or IP address
/// * `port` - Proxy server port
/// * `timeout_secs` - Connection timeout in seconds (default 5)
///
/// # Returns
/// * `TestResultDto` containing success status, latency, and error details
#[command]
pub async fn test_proxy(
    proxy_type: String,
    host: String,
    port: u16,
    timeout_secs: Option<u64>,
) -> Result<TestResultDto, String> {
    let start = std::time::Instant::now();
    let proxy_type_clone = proxy_type.clone();

    info!(
        proxy_type = %proxy_type,
        %host,
        %port,
        "Testing proxy connection"
    );

    // Validate input
    if host.trim().is_empty() {
        return Ok(TestResultDto {
            success: false,
            latency_ms: None,
            mode: proxy_type_clone.clone(),
            error: Some("Proxy host cannot be empty".to_string()),
        });
    }

    // Parse proxy type
    let proxy_scheme = match proxy_type.to_lowercase().as_str() {
        "http" => "http",
        "socks4" => "socks4",
        "socks5" => "socks5",
        _ => {
            return Ok(TestResultDto {
                success: false,
                latency_ms: None,
                mode: proxy_type_clone.clone(),
                error: Some(format!("Unsupported proxy type: {}", proxy_type_clone)),
            });
        }
    };

    // Build proxy URL
    let proxy_url = format!("{}://{}:{}", proxy_scheme, host, port);

    // Build reqwest proxy client
    let proxy = match reqwest::Proxy::all(&proxy_url) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "Failed to create proxy");
            return Ok(TestResultDto {
                success: false,
                latency_ms: None,
                mode: proxy_type_clone.clone(),
                error: Some(format!("Invalid proxy configuration: {}", e)),
            });
        }
    };

    // Build HTTP client with proxy and timeout
    let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(5));
    let client = match reqwest::Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to build HTTP client");
            return Ok(TestResultDto {
                success: false,
                latency_ms: None,
                mode: proxy_type_clone.clone(),
                error: Some(format!("Failed to build client: {}", e)),
            });
        }
    };

    // Test connection to Telegram
    let test_result = match client
        .get("https://telegram.org")
        .send()
        .await
    {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = response.status();

            if status.is_success() {
                info!(
                    latency_ms = latency,
                    status = %status,
                    "Proxy connection successful"
                );
                TestResultDto {
                    success: true,
                    latency_ms: Some(latency),
                    mode: proxy_type_clone.clone(),
                    error: None,
                }
            } else {
                warn!(
                    latency_ms = latency,
                    status = %status,
                    "Proxy connection returned non-success status"
                );
                TestResultDto {
                    success: false,
                    latency_ms: Some(latency),
                    mode: proxy_type_clone.clone(),
                    error: Some(format!("HTTP status: {}", status)),
                }
            }
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            error!(
                error = %e,
                latency_ms = latency,
                "Proxy connection failed"
            );

            // Provide user-friendly error messages
            let error_msg = if e.is_timeout() {
                "Connection timed out".to_string()
            } else if e.is_connect() {
                format!("Connection failed: {}", e)
            } else {
                format!("Request failed: {}", e)
            };

            TestResultDto {
                success: false,
                latency_ms: Some(latency),
                mode: proxy_type_clone.clone(),
                error: Some(error_msg),
            }
        }
    };

    Ok(test_result)
}

// ============================================================================
// Proxy Settings Commands
// ============================================================================

/// Get all proxy settings
///
/// Returns the current proxy configuration including URL and type.
#[command]
pub fn get_proxy_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<ProxySettingsDto, String> {
    let _settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let proxy_url = settings_manager.get_proxy_url();
    let proxy_type = settings_manager.get_proxy_type();

    Ok(ProxySettingsDto {
        proxy_url,
        proxy_type: proxy_type.into(),
    })
}

/// Set proxy URL and type
///
/// Updates the unified proxy settings with the specified URL and proxy type.
///
/// # Arguments
/// * `url` - Proxy URL (e.g., socks5://host:port, socks4://user:pass@host:port, http://host:port)
/// * `proxy_type` - Type of proxy (Socks5, Socks4, Http)
#[command]
pub fn set_proxy_url(
    settings_manager: State<'_, SettingsManager>,
    url: String,
    proxy_type: ProxyType,
) -> Result<(), String> {
    info!(url, ?proxy_type, "Setting proxy URL");

    // Validate URL format
    if url.trim().is_empty() {
        return Err("Proxy URL cannot be empty".to_string());
    }

    // Basic validation of URL format
    if !url.starts_with("socks5://") && !url.starts_with("socks4://") && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Proxy URL must start with socks5://, socks4://, http://, or https://".to_string());
    }

    settings_manager.set_proxy_url(url, proxy_type)
        .map_err(|e| format!("Failed to save proxy settings: {}", e))?;

    info!("Proxy URL updated successfully");
    Ok(())
}

/// Set OpenAI use proxy flag
///
/// Enables or disables OpenAI's use of the unified proxy settings.
///
/// # Arguments
/// * `enabled` - Whether OpenAI should use the unified proxy
#[command]
pub fn set_openai_use_proxy(
    settings_manager: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    info!(enabled, "Setting OpenAI use proxy");

    settings_manager.set_openai_use_proxy(enabled)
        .map_err(|e| format!("Failed to save OpenAI proxy setting: {}", e))?;

    info!("OpenAI proxy setting updated successfully");
    Ok(())
}

/// Set Telegram proxy mode
///
/// Updates the Telegram proxy mode (None, Socks5).
///
/// # Arguments
/// * `mode` - Proxy mode for Telegram connection
#[command]
pub fn set_telegram_proxy_mode(
    settings_manager: State<'_, SettingsManager>,
    mode: ProxyMode,
) -> Result<(), String> {
    info!(?mode, "Setting Telegram proxy mode");

    settings_manager.set_telegram_proxy_mode(mode)
        .map_err(|e| format!("Failed to save Telegram proxy mode: {}", e))?;

    info!("Telegram proxy mode updated successfully");
    Ok(())
}

/// Get current Telegram proxy status
///
/// Returns the cached proxy status including mode and URL.
/// Uses cached value for fast response without locking the client.
///
/// # Returns
/// * Proxy status with mode and URL information
#[command]
pub async fn get_telegram_proxy_status(
    telegram_state: State<'_, TelegramState>,
) -> Result<ProxyStatus, String> {
    // Return cached status (fast, no client lock)
    Ok(telegram_state.get_proxy_status_cached().await)
}

/// Reconnect Telegram with new proxy settings
///
/// Disconnects and reconnects the Telegram client using the current proxy settings.
///
/// # Returns
/// * Status message indicating the operation result
#[command]
pub async fn reconnect_telegram(
    telegram_state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<String, String> {
    info!("Reconnecting Telegram with new proxy settings");

    // Load current settings
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Get proxy mode and determine proxy URL
    let proxy_url = match settings.tts.telegram.proxy_mode {
        ProxyMode::None => None,
        ProxyMode::Socks5 => settings.tts.network.proxy.proxy_url.clone(),
    };

    info!(proxy_mode = ?settings.tts.telegram.proxy_mode, has_proxy = proxy_url.is_some(), "Reconnecting with proxy settings");

    // Check if we have a saved api_id
    let api_id = match settings_manager.get_telegram_api_id() {
        Some(id) => id as u32,
        None => {
            warn!("No saved Telegram session found, cannot reconnect");
            return Ok("No active Telegram session to reconnect".to_string());
        }
    };

    // Create new client and initialize with proxy
    let client = crate::telegram::TelegramClient::new();
    info!("Initializing new Telegram client with proxy settings");

    client.init_empty_with_proxy(api_id, proxy_url).await
        .map_err(|e| format!("Failed to reconnect: {}", e))?;

    // Replace old client with new one in a single lock
    let (is_authorized, proxy_status) = {
        let mut client_guard = telegram_state.client.lock().await;

        // Disconnect and take old client if exists
        if let Some(old_client) = mem::replace(&mut *client_guard, None) {
            info!("Disconnecting existing Telegram client");
            if let Err(e) = old_client.disconnect().await {
                warn!("Failed to disconnect old client: {}", e);
            }
        }

        // Save new client
        *client_guard = Some(client);

        // Get proxy status and verify authorization while holding lock
        let status = if let Some(c) = client_guard.as_ref() {
            c.get_proxy_status().await
        } else {
            crate::telegram::ProxyStatus::default()
        };

        let authorized = if let Some(c) = client_guard.as_ref() {
            c.is_authorized().await?
        } else {
            false
        };

        (authorized, status)
    };

    // Update cached proxy status
    telegram_state.update_proxy_status(proxy_status.clone()).await;

    if is_authorized {
        info!(status = %proxy_status, "Telegram reconnected successfully");
        Ok(format!("Reconnected successfully: {}", proxy_status))
    } else {
        warn!("Telegram reconnected but not authorized");
        Ok("Reconnected but session is not authorized".to_string())
    }
}
