//! UPnP port forwarding module
//!
//! Provides automatic port forwarding on UPnP-enabled routers.

use igd::{search_gateway, Gateway, PortMappingProtocol, SearchOptions};
use std::sync::{Mutex, Arc};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};

/// UPnP manager for automatic port forwarding
///
/// Automatically opens the configured port on the router when created,
/// and closes it when dropped. If UPnP is not available, operations
/// gracefully fail with warnings.
pub struct UpnpManager {
    port: u16,
    gateway: Arc<Mutex<Option<Gateway>>>,
}

impl UpnpManager {
    /// Create a new UPnP manager for the given port
    ///
    /// Attempts to discover UPnP devices on the local network.
    /// If no devices are found, operations will gracefully fail.
    pub fn new(port: u16) -> Self {
        tracing::info!("UPnP manager created for port {}", port);
        Self {
            port,
            gateway: Arc::new(Mutex::new(None)),
        }
    }

    /// Discover UPnP gateway on the local network
    fn discover_gateway(&self) -> Result<(), String> {
        let mut gw = self.gateway.lock()
            .map_err(|e| format!("Failed to lock gateway: {}", e))?;

        if gw.is_some() {
            return Ok(());
        }

        tracing::info!("Searching for UPnP gateway...");
        let gateway = search_gateway(SearchOptions::default())
            .map_err(|e| {
                tracing::warn!(error = %e, "UPnP gateway search failed");
                format!("UPnP gateway not found: {}", e)
            })?;

        let addr = gateway.addr;
        tracing::info!(gateway_addr = %addr, "UPnP gateway found");
        *gw = Some(gateway);
        Ok(())
    }

    /// Get local IP address that can reach the gateway
    ///
    /// Uses a UDP trick to find the local IP of the interface
    /// that can reach the gateway.
    fn get_local_ip(&self) -> Result<Ipv4Addr, String> {
        // Use UDP connection to a reliable external address
        // This returns the local IP of the correct interface
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;

        socket.connect("8.8.8.8:80")
            .map_err(|e| format!("Failed to connect to external address: {}", e))?;

        let local_addr = socket.local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?;

        match local_addr.ip() {
            IpAddr::V4(ip) => Ok(ip),
            IpAddr::V6(_) => Err("Got IPv6 address, expected IPv4".to_string()),
        }
    }

    /// Forward the configured port on the router
    ///
    /// Opens the external port on the UPnP gateway to redirect
    /// to the same port on this machine. Uses TCP protocol.
    pub fn forward(&self) -> Result<(), String> {
        // Discover gateway if not already done
        if self.gateway.lock().map_err(|e| format!("Failed to lock gateway: {}", e))?.is_none() {
            self.discover_gateway()?;
        }

        // Get local IP first (before locking gateway)
        let local_ip = self.get_local_ip()?;

        // Now lock gateway for the add_port call
        let gw = self.gateway.lock()
            .map_err(|e| format!("Failed to lock gateway: {}", e))?;

        let gateway = gw.as_ref().unwrap();
        let local_addr = SocketAddrV4::new(local_ip, self.port);

        // Duration in seconds (1 hour lease)
        let duration = 3600u32;

        tracing::info!(
            external_port = self.port,
            local_addr = %local_addr,
            "Adding UPnP port mapping"
        );

        gateway.add_port(
            PortMappingProtocol::TCP,
            self.port,
            local_addr,
            duration,
            "ttsbard-webview",
        ).map_err(|e| {
            tracing::warn!(error = %e, "Failed to add UPnP port mapping");
            format!("Failed to add port mapping: {}", e)
        })?;

        tracing::info!(port = self.port, "UPnP port forwarding enabled");
        Ok(())
    }

    /// Remove the port forwarding from the router
    ///
    /// Closes the external port mapping. This is called automatically
    /// on drop, but can also be called manually if needed.
    fn remove_internal(&self, context: &str) {
        let gw = self.gateway.lock();
        if let Ok(gateway) = gw {
            if let Some(g) = gateway.as_ref() {
                tracing::debug!(port = self.port, context, "Removing UPnP port mapping");
                if let Err(e) = g.remove_port(PortMappingProtocol::TCP, self.port) {
                    tracing::warn!(error = %e, port = self.port, context, "Failed to remove UPnP port mapping");
                } else {
                    tracing::info!(port = self.port, context, "UPnP port mapping removed");
                }
            }
        }
    }

    /// Remove the port forwarding from the router
    ///
    /// Closes the external port mapping. This is called automatically
    /// on drop, but can also be called manually if needed.
    pub fn remove(&self) {
        self.remove_internal("manual");
    }
}

impl Drop for UpnpManager {
    fn drop(&mut self) {
        self.remove_internal("drop");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upnp_manager_creation() {
        let _manager = UpnpManager::new(10100);
        // Test that manager can be created without panicking
    }
}
