use serde::{Deserialize, Serialize};
use tauri::{command, State};
use tracing::{info, error, warn};
use std::time::Instant;
use crate::config::{SettingsManager, dto::ProxySettingsDto, ProxyType, ProxyMode, MtProxySettings};
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

    // Check if we have a saved api_id
    let api_id = match settings_manager.get_telegram_api_id() {
        Some(id) => id as u32,
        None => {
            warn!("No saved Telegram session found, cannot reconnect");
            return Ok("No active Telegram session to reconnect".to_string());
        }
    };

    // Create new client and initialize with appropriate proxy settings
    let client = crate::telegram::TelegramClient::new();
    info!(proxy_mode = ?settings.tts.telegram.proxy_mode, "Initializing new Telegram client with proxy settings");

    // Initialize client based on proxy mode
    match &settings.tts.telegram.proxy_mode {
        ProxyMode::None => {
            info!("Initializing without proxy");
            client.init_empty(api_id).await
                .map_err(|e| format!("Failed to reconnect: {}", e))?;
        }
        ProxyMode::Socks5 => {
            let proxy_url = settings.tts.network.proxy.proxy_url.clone();
            info!(has_proxy = proxy_url.is_some(), "Initializing with SOCKS5 proxy");
            client.init_empty_with_proxy(api_id, proxy_url).await
                .map_err(|e| format!("Failed to reconnect: {}", e))?;
        }
        ProxyMode::MtProxy => {
            #[cfg(feature = "mtproxy")]
            {
                info!("Initializing with MTProxy");
                client.init_empty_with_mtproxy(api_id, &settings.tts.network.mtproxy).await
                    .map_err(|e| format!("Failed to reconnect: {}", e))?;
            }
            #[cfg(not(feature = "mtproxy"))]
            {
                return Err("MTProxy feature is not enabled".to_string());
            }
        }
    }

    // Replace old client with new one in a single lock
    let (is_authorized, proxy_status) = {
        let mut client_guard = telegram_state.client.lock().await;

        // Disconnect and take old client if exists
        if let Some(old_client) = (*client_guard).take() {
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

// ============================================================================
// MTProxy Commands
// ============================================================================

/// Validate MTProxy secret format
///
/// Validates that the secret is either:
/// - Hex (32 chars for 16 bytes, or 34 chars with dd/ee prefix)
/// - Base64 (24 chars)
fn validate_mtproxy_secret(secret: &str) -> Result<(), String> {
    let secret = secret.trim();
    let len = secret.len();

    // Base64 format (24 chars = 16 bytes encoded)
    if len == 24 {
        // Try to decode as base64 using Engine
        use base64::Engine;
        if base64::engine::general_purpose::STANDARD.decode(secret).is_ok() {
            return Ok(());
        }
    }

    // Hex format (32 chars = 16 bytes, or 34 chars with prefix)
    if len == 32 || len == 34 {
        // Check for prefix
        if len == 34 {
            let prefix = &secret[..2].to_lowercase();
            if prefix != "dd" && prefix != "ee" {
                return Err("Secret prefix must be 'dd' or 'ee' for 34-character hex".to_string());
            }
        }

        // Try to decode as hex
        if hex::decode(&secret[2..]).or_else(|_| hex::decode(secret)).is_ok() {
            return Ok(());
        }
    }

    Err(format!(
        "Invalid secret format. Expected hex (32 or 34 chars) or base64 (24 chars), got {} chars",
        len
    ))
}

/// Test MTProxy connection to Telegram
///
/// This command tests an MTProxy connection by attempting to connect
/// to Telegram through the specified MTProxy server.
///
/// # Arguments
/// * `host` - MTProxy server hostname or IP address
/// * `port` - MTProxy server port (typically 8888)
/// * `secret` - MTProxy secret key (hex or base64 encoded)
/// * `dc_id` - Optional DC ID (data center ID)
/// * `timeout_secs` - Connection timeout in seconds (default 10)
///
/// # Returns
/// * `TestResultDto` containing success status, latency, and error details
#[command]
pub async fn test_mtproxy(
    host: String,
    port: u16,
    secret: String,
    dc_id: Option<i32>,
    timeout_secs: Option<u64>,
) -> Result<TestResultDto, String> {
    let start = Instant::now();

    info!(
        %host,
        %port,
        secret_preview = &secret[..secret.len().min(4)],
        ?dc_id,
        "Testing MTProxy connection"
    );

    // Validate input
    if host.trim().is_empty() {
        return Ok(TestResultDto {
            success: false,
            latency_ms: None,
            mode: "mtproxy".to_string(),
            error: Some("MTProxy host cannot be empty".to_string()),
        });
    }

    // Validate port range
    if port == 0 {
        return Ok(TestResultDto {
            success: false,
            latency_ms: None,
            mode: "mtproxy".to_string(),
            error: Some("Port cannot be 0".to_string()),
        });
    }

    // Validate secret format
    if let Err(e) = validate_mtproxy_secret(&secret) {
        return Ok(TestResultDto {
            success: false,
            latency_ms: None,
            mode: "mtproxy".to_string(),
            error: Some(format!("Invalid secret: {}", e)),
        });
    }

    // For MTProxy, we need to test with the actual Telegram protocol
    // which requires API credentials. For now, we'll do a basic TCP connection
    // test to verify the MTProxy server is reachable.
    //
    // Full MTProxy testing requires Telegram client initialization which
    // is done in the actual connection process.

    let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(10));

    // Test TCP connection to MTProxy server
    let addr = format!("{}:{}", host, port);
    let test_result = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(&addr)
    ).await {
        Ok(Ok(_stream)) => {
            let latency = start.elapsed().as_millis() as u64;
            info!(
                latency_ms = latency,
                "MTProxy server is reachable"
            );
            TestResultDto {
                success: true,
                latency_ms: Some(latency),
                mode: "mtproxy".to_string(),
                error: None,
            }
        }
        Ok(Err(e)) => {
            let latency = start.elapsed().as_millis() as u64;
            error!(
                error = %e,
                latency_ms = latency,
                "Failed to connect to MTProxy server"
            );
            TestResultDto {
                success: false,
                latency_ms: Some(latency),
                mode: "mtproxy".to_string(),
                error: Some(format!("Connection failed: {}", e)),
            }
        }
        Err(_) => {
            let latency = start.elapsed().as_millis() as u64;
            error!(
                latency_ms = latency,
                "MTProxy connection timed out"
            );
            TestResultDto {
                success: false,
                latency_ms: Some(latency),
                mode: "mtproxy".to_string(),
                error: Some("Connection timed out".to_string()),
            }
        }
    };

    Ok(test_result)
}

/// Get MTProxy settings
///
/// Returns the current MTProxy configuration.
#[command]
pub fn get_mtproxy_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<MtProxySettings, String> {
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    Ok(settings.tts.network.mtproxy)
}

/// Set MTProxy settings
///
/// Updates the MTProxy configuration with the specified parameters.
///
/// # Arguments
/// * `host` - MTProxy server host (IP or domain)
/// * `port` - MTProxy server port
/// * `secret` - MTProxy secret key (hex or base64 encoded)
/// * `dc_id` - Optional DC ID (data center ID)
#[command]
pub fn set_mtproxy_settings(
    settings_manager: State<'_, SettingsManager>,
    host: Option<String>,
    port: u16,
    secret: Option<String>,
    dc_id: Option<i32>,
) -> Result<(), String> {
    info!(
        ?host,
        %port,
        has_secret = secret.is_some(),
        ?dc_id,
        "Setting MTProxy configuration"
    );

    // Validate host if provided
    if let Some(ref h) = host {
        if h.trim().is_empty() {
            return Err("MTProxy host cannot be empty".to_string());
        }
    }

    // Validate secret if provided
    if let Some(ref s) = secret {
        if !s.trim().is_empty() {
            validate_mtproxy_secret(s)
                .map_err(|e| format!("Invalid secret: {}", e))?;
        }
    }

    settings_manager.set_mtproxy_settings(host, port, secret, dc_id)
        .map_err(|e| format!("Failed to save MTProxy settings: {}", e))?;

    info!("MTProxy settings updated successfully");
    Ok(())
}
