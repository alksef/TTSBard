//! WebView security module
//!
//! Provides security functions for token authentication, network detection,
//! and constant-time comparisons to prevent timing attacks.

use std::net::IpAddr;
use subtle::ConstantTimeEq;

/// Check if an IP address is from a local/private network
///
/// Returns true for:
/// - IPv4 loopback (127.0.0.0/8)
/// - IPv4 private networks (192.168.0.0/16, 10.0.0.0/8, 172.16.0.0/12)
/// - IPv4 link-local (169.254.0.0/16)
/// - IPv6 loopback (::1)
/// - IPv6 unique local (fc00::/7)
pub fn is_local_network(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) if addr.is_loopback() => true,
        IpAddr::V4(addr) if addr.is_private() => true,
        IpAddr::V4(addr) if addr.is_link_local() => true,
        IpAddr::V6(addr) if addr.is_loopback() => true,
        IpAddr::V6(addr) => (addr.segments()[0] & 0xfe00) == 0xfc00,
        _ => false,
    }
}

/// Validate a token using constant-time comparison
///
/// This prevents timing attacks by using the subtle crate's ConstantTimeEq trait.
/// Returns true if:
/// - Both provided and stored tokens are None (no token configured)
/// - Both tokens match exactly
pub fn validate_token(provided: Option<&str>, stored: Option<&str>) -> bool {
    match (provided, stored) {
        (Some(p), Some(s)) => {
            // Constant-time comparison handles different lengths correctly
            bool::from(p.as_bytes().ct_eq(s.as_bytes()))
        }
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_local_network_ipv4_loopback() {
        assert!(is_local_network("127.0.0.1".parse().unwrap()));
        assert!(is_local_network("127.0.0.2".parse().unwrap()));
    }

    #[test]
    fn test_is_local_network_ipv4_private() {
        assert!(is_local_network("192.168.1.1".parse().unwrap()));
        assert!(is_local_network("10.0.0.1".parse().unwrap()));
        assert!(is_local_network("172.16.0.1".parse().unwrap()));
    }

    #[test]
    fn test_is_local_network_ipv4_public() {
        assert!(!is_local_network("8.8.8.8".parse().unwrap()));
        assert!(!is_local_network("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn test_validate_token_match() {
        assert!(validate_token(Some("secret"), Some("secret")));
    }

    #[test]
    fn test_validate_token_no_match() {
        assert!(!validate_token(Some("wrong"), Some("secret")));
    }

    #[test]
    fn test_validate_token_none() {
        assert!(validate_token(None, None));
    }

    #[test]
    fn test_validate_token_one_none() {
        assert!(!validate_token(Some("token"), None));
        assert!(!validate_token(None, Some("token")));
    }
}
