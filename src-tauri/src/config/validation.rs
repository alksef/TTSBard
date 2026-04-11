//! Validation utilities for configuration values

use std::fmt;

/// Check if a string is a valid hex color (#RRGGBB format)
pub fn is_valid_hex_color(color: &str) -> bool {
    if color.len() != 7 {
        return false;
    }
    if !color.starts_with('#') {
        return false;
    }
    color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate and clamp opacity value (10-100)
pub fn validate_opacity(opacity: u8) -> u8 {
    opacity.clamp(10, 100)
}

/// Validate and clamp volume value (0-100)
pub fn validate_volume(volume: u8) -> u8 {
    volume.clamp(0, 100)
}

/// Validate port number (1024-65535)
pub fn validate_port(port: u16) -> Result<u16, ConfigError> {
    if port < 1024 {
        Err(ConfigError::Port("Port must be >= 1024".to_string()))
    } else {
        Ok(port)
    }
}

/// Configuration validation errors
#[derive(Debug, Clone)]
pub enum ConfigError {
    Port(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Port(msg) => write!(f, "Invalid port: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_hex_color() {
        assert!(is_valid_hex_color("#1e1e1e"));
        assert!(is_valid_hex_color("#ffffff"));
        assert!(is_valid_hex_color("#000000"));
        assert!(!is_valid_hex_color("#1e1e1")); // too short
        assert!(!is_valid_hex_color("#1e1e1e1")); // too long
        assert!(!is_valid_hex_color("1e1e1e")); // missing #
        assert!(!is_valid_hex_color("#gggggg")); // invalid hex
    }

    #[test]
    fn test_validate_opacity() {
        assert_eq!(validate_opacity(0), 10);
        assert_eq!(validate_opacity(50), 50);
        assert_eq!(validate_opacity(100), 100);
        assert_eq!(validate_opacity(200), 100);
    }

    #[test]
    fn test_validate_volume() {
        assert_eq!(validate_volume(0), 0);
        assert_eq!(validate_volume(50), 50);
        assert_eq!(validate_volume(100), 100);
        assert_eq!(validate_volume(200), 100);
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(1024).is_ok());
        assert!(validate_port(8080).is_ok());
        assert!(validate_port(65535).is_ok());
        assert!(validate_port(0).is_err());
        assert!(validate_port(1023).is_err());
    }
}
