use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info, warn};

use crate::config::{validate_port, validate_vrchat_host, VrchatSettings};
use crate::vrchat::transport::VrchatOscTransport;

/// Resolves a host and port pair into a validated `SocketAddr`.
pub fn resolve_vrchat_destination(host: &str, port: u16) -> Result<SocketAddr, String> {
    validate_vrchat_host(host)?;
    validate_port(port).map_err(|e| e.to_string())?;

    let host_trimmed = host.trim();
    let addr_str = format!("{}:{}", host_trimmed, port);
    if let Ok(addr) = addr_str.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let mut addrs = (host_trimmed, port)
        .to_socket_addrs()
        .map_err(|e| format!("Cannot resolve destination '{host_trimmed}:{port}': {e}"))?;
    addrs
        .next()
        .ok_or_else(|| format!("No address resolved for '{host_trimmed}:{port}'"))
}

/// Runtime service managing outbound VRChat OSC integration.
#[derive(Clone)]
pub struct VrchatService {
    pub settings: Arc<RwLock<VrchatSettings>>,
    transport: Arc<RwLock<Option<Arc<VrchatOscTransport>>>>,
    published_typing: Arc<AtomicBool>,
    operation_lock: Arc<tokio::sync::Mutex<()>>,
}

impl VrchatService {
    /// Creates a new service with default settings.
    pub fn new() -> Self {
        Self::with_settings(VrchatSettings::default())
    }

    /// Creates a new service with specified settings.
    pub fn with_settings(settings: VrchatSettings) -> Self {
        Self {
            settings: Arc::new(RwLock::new(settings)),
            transport: Arc::new(RwLock::new(None)),
            published_typing: Arc::new(AtomicBool::new(false)),
            operation_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Returns whether the VRChat integration is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.settings.read().enabled
    }

    /// Returns whether a typing indicator is currently published as active.
    pub fn published_typing(&self) -> bool {
        self.published_typing.load(Ordering::SeqCst)
    }

    /// Explicitly updates the published typing state tracker.
    pub fn set_published_typing(&self, val: bool) {
        self.published_typing.store(val, Ordering::SeqCst);
    }

    /// Synchronously updates the settings cache and transport destination (e.g. from SettingsManager).
    pub fn update_settings_internal(&self, settings: VrchatSettings) {
        match resolve_vrchat_destination(&settings.host, settings.port) {
            Ok(dest) => {
                let mut transport = self.transport.write();
                if let Some(existing) = transport.as_ref() {
                    if existing.destination().is_ipv6() == dest.is_ipv6() {
                        existing.set_destination(dest);
                    } else {
                        *transport = None;
                    }
                }
                *self.settings.write() = settings;
            }
            Err(error) => {
                warn!(%error, "Invalid saved VRChat destination; disabling OSC output");
                let mut disabled = settings;
                disabled.enabled = false;
                *self.settings.write() = disabled;
                *self.transport.write() = None;
                self.published_typing.store(false, Ordering::SeqCst);
            }
        }
    }

    /// Returns the active transport or creates one for the current destination.
    async fn get_or_init_transport(&self) -> Result<Arc<VrchatOscTransport>, String> {
        if let Some(transport) = self.transport.read().clone() {
            return Ok(transport);
        }

        let (host, port) = {
            let s = self.settings.read();
            (s.host.clone(), s.port)
        };

        let dest = resolve_vrchat_destination(&host, port)?;

        let transport = Arc::new(
            VrchatOscTransport::new(dest)
                .await
                .map_err(|e| e.to_string())?,
        );
        *self.transport.write() = Some(transport.clone());
        Ok(transport)
    }

    /// Sends typing indicator to VRChat if enabled.
    /// Fails with an error if the integration is disabled.
    pub async fn set_typing(&self, typing: bool) -> Result<(), String> {
        let _guard = self.operation_lock.lock().await;
        if !self.is_enabled() {
            return Err("VRChat integration is disabled".to_string());
        }

        let transport = self.get_or_init_transport().await?;
        transport
            .send_typing(typing)
            .await
            .map_err(|e| e.to_string())?;
        self.published_typing.store(typing, Ordering::SeqCst);
        debug!(typing, "Sent VRChat chatbox typing state");
        Ok(())
    }

    /// Sends final text to VRChat chatbox if enabled.
    /// Returns `Ok(true)` if sent, `Ok(false)` if text was empty/whitespace,
    /// or `Err` if disabled or transport failed.
    pub async fn send_text(&self, text: &str) -> Result<bool, String> {
        let _guard = self.operation_lock.lock().await;
        if !self.is_enabled() {
            return Err("VRChat integration is disabled".to_string());
        }

        let transport = self.get_or_init_transport().await?;
        if self.published_typing() {
            transport
                .send_typing(false)
                .await
                .map_err(|e| e.to_string())?;
            self.set_published_typing(false);
        }
        let sent = transport.send_text(text).await.map_err(|e| e.to_string())?;
        if sent {
            debug!("Sent final text to VRChat chatbox");
        }
        Ok(sent)
    }

    /// Validates and applies new settings.
    /// If transitioning from enabled to disabled while `published_typing` was true,
    /// an attempt to send typing false is made before disabling.
    pub async fn apply_settings(&self, new_settings: VrchatSettings) -> Result<(), String> {
        let dest = resolve_vrchat_destination(&new_settings.host, new_settings.port)?;
        self.apply_settings_resolved(new_settings, dest).await;
        Ok(())
    }

    /// Apply a destination validated before the settings were persisted.
    pub async fn apply_settings_resolved(&self, new_settings: VrchatSettings, dest: SocketAddr) {
        let _guard = self.operation_lock.lock().await;

        let was_enabled = self.is_enabled();
        let old_destination = self.transport.read().as_ref().map(|t| t.destination());
        if was_enabled
            && self.published_typing()
            && (!new_settings.enabled || old_destination.is_some_and(|old| old != dest))
        {
            info!("VRChat typing destination changed or integration disabled — clearing old indicator");
            if let Ok(transport) = self.get_or_init_transport().await {
                if let Err(e) = transport.send_typing(false).await {
                    warn!(error = %e, "Failed to send typing false on VRChat disable");
                }
            }
            self.set_published_typing(false);
        }

        let mut transport = self.transport.write();
        if let Some(existing) = transport.as_ref() {
            if existing.destination().is_ipv6() == dest.is_ipv6() {
                existing.set_destination(dest);
            } else {
                *transport = None;
            }
        }

        *self.settings.write() = new_settings;
    }
}

impl Default for VrchatService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::decoder::decode_udp;
    use rosc::{OscPacket, OscType};
    use std::time::Duration;
    use tokio::net::UdpSocket;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_disabled_commands_refuse_to_send() {
        let service = VrchatService::new();
        assert!(!service.is_enabled());

        // Commands must refuse to send when disabled
        let typing_err = service.set_typing(true).await;
        assert!(typing_err.is_err(), "set_typing must fail when disabled");

        let text_err = service.send_text("Hello").await;
        assert!(text_err.is_err(), "send_text must fail when disabled");
    }

    #[tokio::test]
    async fn test_enabled_service_sends_packets_and_tracks_typing() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.expect("bind receiver");
        let recv_addr = receiver.local_addr().expect("local addr");

        let service = VrchatService::with_settings(VrchatSettings {
            enabled: true,
            start_on_boot: false,
            host: recv_addr.ip().to_string(),
            port: recv_addr.port(),
        });
        assert!(service.is_enabled());
        assert!(!service.published_typing());

        // 1. Send typing true
        service.set_typing(true).await.expect("set typing true");
        assert!(service.published_typing());

        let mut buf = [0u8; 1024];
        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout")
            .expect("recv");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");
        if let OscPacket::Message(msg) = packet {
            assert_eq!(msg.addr, "/chatbox/typing");
            assert_eq!(msg.args[0], OscType::Bool(true));
        } else {
            panic!("expected OscMessage");
        }

        // 2. Final text clears typing before the chatbox input.
        let sent = service.send_text("Testing 123").await.expect("send text");
        assert!(sent);

        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout")
            .expect("recv");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");
        if let OscPacket::Message(msg) = packet {
            assert_eq!(msg.addr, "/chatbox/typing");
            assert_eq!(msg.args[0], OscType::Bool(false));
        } else {
            panic!("expected OscMessage");
        }
        assert!(!service.published_typing());

        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout")
            .expect("recv");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");
        if let OscPacket::Message(msg) = packet {
            assert_eq!(msg.addr, "/chatbox/input");
            assert_eq!(msg.args[0], OscType::String("Testing 123".to_string()));
        } else {
            panic!("expected OscMessage");
        }
    }

    #[tokio::test]
    async fn test_disable_attempts_typing_false_if_published_typing() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.expect("bind receiver");
        let recv_addr = receiver.local_addr().expect("local addr");

        let service = VrchatService::with_settings(VrchatSettings {
            enabled: true,
            start_on_boot: false,
            host: recv_addr.ip().to_string(),
            port: recv_addr.port(),
        });

        // Set typing true
        service.set_typing(true).await.expect("set typing true");
        assert!(service.published_typing());

        let mut buf = [0u8; 1024];
        let _ = receiver
            .recv_from(&mut buf)
            .await
            .expect("drain typing true");

        // Now disable via apply_settings
        let new_settings = VrchatSettings {
            enabled: false,
            start_on_boot: false,
            host: recv_addr.ip().to_string(),
            port: recv_addr.port(),
        };
        service
            .apply_settings(new_settings)
            .await
            .expect("apply settings");

        // Verify published_typing is now false
        assert!(!service.published_typing());
        assert!(!service.is_enabled());

        // Verify that typing false packet was received over UDP
        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout waiting for typing false packet")
            .expect("recv error");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");
        if let OscPacket::Message(msg) = packet {
            assert_eq!(msg.addr, "/chatbox/typing");
            assert_eq!(msg.args[0], OscType::Bool(false));
        } else {
            panic!("expected OscMessage");
        }
    }

    #[tokio::test]
    async fn changing_destination_clears_typing_at_the_old_address() {
        let old_receiver = UdpSocket::bind("127.0.0.1:0").await.expect("old receiver");
        let new_receiver = UdpSocket::bind("127.0.0.1:0").await.expect("new receiver");
        let old_addr = old_receiver.local_addr().expect("old address");
        let new_addr = new_receiver.local_addr().expect("new address");
        let service = VrchatService::with_settings(VrchatSettings {
            enabled: true,
            start_on_boot: false,
            host: old_addr.ip().to_string(),
            port: old_addr.port(),
        });
        service.set_typing(true).await.expect("typing true");
        let mut buf = [0u8; 1024];
        old_receiver
            .recv_from(&mut buf)
            .await
            .expect("drain typing true");

        service
            .apply_settings(VrchatSettings {
                enabled: true,
                start_on_boot: false,
                host: new_addr.ip().to_string(),
                port: new_addr.port(),
            })
            .await
            .expect("change address");
        let (len, _) = timeout(Duration::from_secs(2), old_receiver.recv_from(&mut buf))
            .await
            .expect("old receiver timeout")
            .expect("old receiver read");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");
        match packet {
            OscPacket::Message(msg) => {
                assert_eq!(msg.addr, "/chatbox/typing");
                assert_eq!(msg.args[0], OscType::Bool(false));
            }
            _ => panic!("expected OSC message"),
        }
        assert!(!service.published_typing());

        service.send_text("new address").await.expect("send text");
        let (len, _) = timeout(Duration::from_secs(2), new_receiver.recv_from(&mut buf))
            .await
            .expect("new receiver timeout")
            .expect("new receiver read");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");
        match packet {
            OscPacket::Message(msg) => assert_eq!(msg.addr, "/chatbox/input"),
            _ => panic!("expected OSC message"),
        }
    }

    #[test]
    fn test_resolve_vrchat_destination_validation() {
        assert!(resolve_vrchat_destination("127.0.0.1", 9000).is_ok());
        assert!(resolve_vrchat_destination("localhost", 9000).is_ok());

        // Invalid port
        assert!(resolve_vrchat_destination("127.0.0.1", 80).is_err());
        assert!(resolve_vrchat_destination("127.0.0.1", 0).is_err());

        // Invalid host
        assert!(resolve_vrchat_destination("", 9000).is_err());
        assert!(resolve_vrchat_destination("http://127.0.0.1", 9000).is_err());
        assert!(resolve_vrchat_destination("127.0.0.1:9000", 9000).is_err());
    }
}
