use parking_lot::RwLock;
use rosc::encoder;
use rosc::{OscMessage, OscPacket, OscType};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

use crate::vrchat::shaping::prepare_chatbox_text;

pub const OSC_ADDR_TYPING: &str = "/chatbox/typing";
pub const OSC_ADDR_INPUT: &str = "/chatbox/input";

#[derive(Debug, thiserror::Error)]
pub enum VrchatOscError {
    #[error("failed to encode OSC packet: {0}")]
    Encode(#[from] rosc::OscError),
    #[error("UDP socket I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Outbound-only OSC transport for VRChat chatbox.
pub struct VrchatOscTransport {
    socket: UdpSocket,
    destination: RwLock<SocketAddr>,
}

impl VrchatOscTransport {
    /// Creates a new transport bound to an ephemeral local port with the given destination.
    pub async fn new(destination: SocketAddr) -> Result<Self, VrchatOscError> {
        let bind_addr = if destination.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };
        let socket = UdpSocket::bind(bind_addr).await?;
        Ok(Self {
            socket,
            destination: RwLock::new(destination),
        })
    }

    /// Creates a new transport from an existing Tokio UdpSocket.
    pub fn from_socket(socket: UdpSocket, destination: SocketAddr) -> Self {
        Self {
            socket,
            destination: RwLock::new(destination),
        }
    }

    /// Returns the current destination address.
    pub fn destination(&self) -> SocketAddr {
        *self.destination.read()
    }

    /// Updates the destination address.
    pub fn set_destination(&self, destination: SocketAddr) {
        *self.destination.write() = destination;
    }

    /// Builds a `/chatbox/typing` OSC message datagram.
    pub fn encode_typing_packet(typing: bool) -> Result<Vec<u8>, VrchatOscError> {
        let msg = OscMessage {
            addr: OSC_ADDR_TYPING.to_string(),
            args: vec![OscType::Bool(typing)],
        };
        encoder::encode(&OscPacket::Message(msg)).map_err(Into::into)
    }

    /// Builds a `/chatbox/input` OSC message datagram if the text is non-empty.
    ///
    /// Parameters:
    /// - `s`: formatted text copy (<= 144 characters)
    /// - `b`: `true` (immediate send)
    /// - `n`: `true` (notification SFX enabled)
    pub fn encode_input_packet(shaped_text: &str) -> Result<Vec<u8>, VrchatOscError> {
        let msg = OscMessage {
            addr: OSC_ADDR_INPUT.to_string(),
            args: vec![
                OscType::String(shaped_text.to_string()),
                OscType::Bool(true),
                OscType::Bool(true),
            ],
        };
        encoder::encode(&OscPacket::Message(msg)).map_err(Into::into)
    }

    /// Sends `/chatbox/typing` indicator to the configured destination.
    pub async fn send_typing(&self, typing: bool) -> Result<(), VrchatOscError> {
        let dest = self.destination();
        let bytes = Self::encode_typing_packet(typing)?;
        self.socket.send_to(&bytes, dest).await?;
        Ok(())
    }

    /// Shapes raw text and, if non-empty, sends `/chatbox/input` to the configured destination.
    /// Returns `Ok(true)` if datagram was sent, or `Ok(false)` if text was empty/whitespace.
    pub async fn send_text(&self, raw_text: &str) -> Result<bool, VrchatOscError> {
        let Some(shaped) = prepare_chatbox_text(raw_text) else {
            return Ok(false);
        };
        let dest = self.destination();
        let bytes = Self::encode_input_packet(&shaped)?;
        self.socket.send_to(&bytes, dest).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::decoder::decode_udp;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_send_typing_true_and_false_over_udp() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.expect("bind receiver");
        let recv_addr = receiver.local_addr().expect("receiver addr");

        let transport = VrchatOscTransport::new(recv_addr)
            .await
            .expect("create transport");
        assert_eq!(transport.destination(), recv_addr);

        let mut buf = [0u8; 1024];

        // 1. Typing true
        transport.send_typing(true).await.expect("send typing true");
        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout waiting for packet")
            .expect("recv error");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode OSC packet");
        match packet {
            OscPacket::Message(msg) => {
                assert_eq!(msg.addr, OSC_ADDR_TYPING);
                assert_eq!(msg.args.len(), 1);
                assert_eq!(msg.args[0], OscType::Bool(true));
            }
            other => panic!("expected OscPacket::Message, got {:?}", other),
        }

        // 2. Typing false
        transport
            .send_typing(false)
            .await
            .expect("send typing false");
        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout waiting for packet")
            .expect("recv error");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode OSC packet");
        match packet {
            OscPacket::Message(msg) => {
                assert_eq!(msg.addr, OSC_ADDR_TYPING);
                assert_eq!(msg.args.len(), 1);
                assert_eq!(msg.args[0], OscType::Bool(false));
            }
            other => panic!("expected OscPacket::Message, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_send_text_cyrillic_and_arguments_over_udp() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.expect("bind receiver");
        let recv_addr = receiver.local_addr().expect("receiver addr");

        let transport = VrchatOscTransport::new(recv_addr)
            .await
            .expect("create transport");

        let mut buf = [0u8; 2048];
        let cyrillic_msg = "Привет, мир! Как дела?";

        let sent = transport
            .send_text(cyrillic_msg)
            .await
            .expect("send cyrillic text");
        assert!(sent);

        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout waiting for packet")
            .expect("recv error");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode OSC packet");

        match packet {
            OscPacket::Message(msg) => {
                assert_eq!(msg.addr, OSC_ADDR_INPUT);
                assert_eq!(msg.args.len(), 3);
                assert_eq!(msg.args[0], OscType::String(cyrillic_msg.to_string()));
                assert_eq!(msg.args[1], OscType::Bool(true)); // b_send: immediate
                assert_eq!(msg.args[2], OscType::Bool(true)); // b_notification: play SFX
            }
            other => panic!("expected OscPacket::Message, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_empty_or_whitespace_text_sends_nothing() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.expect("bind receiver");
        let recv_addr = receiver.local_addr().expect("receiver addr");

        let transport = VrchatOscTransport::new(recv_addr)
            .await
            .expect("create transport");

        let sent_empty = transport.send_text("").await.expect("send empty text");
        assert!(!sent_empty);

        let sent_ws = transport
            .send_text("   \n\t  ")
            .await
            .expect("send whitespace text");
        assert!(!sent_ws);

        let mut buf = [0u8; 512];
        let recv_result = timeout(Duration::from_millis(150), receiver.recv_from(&mut buf)).await;
        assert!(
            recv_result.is_err(),
            "expected timeout because no packet should be transmitted"
        );
    }

    #[tokio::test]
    async fn test_send_text_truncation_over_udp() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.expect("bind receiver");
        let recv_addr = receiver.local_addr().expect("receiver addr");

        let transport = VrchatOscTransport::new(recv_addr)
            .await
            .expect("create transport");

        let long_text = "Тестовая строка ".repeat(15); // > 144 chars
        let sent = transport.send_text(&long_text).await.expect("send text");
        assert!(sent);

        let mut buf = [0u8; 2048];
        let (len, _) = timeout(Duration::from_secs(2), receiver.recv_from(&mut buf))
            .await
            .expect("timeout")
            .expect("recv");
        let (_, packet) = decode_udp(&buf[..len]).expect("decode");

        match packet {
            OscPacket::Message(msg) => {
                assert_eq!(msg.addr, OSC_ADDR_INPUT);
                if let OscType::String(ref s) = msg.args[0] {
                    assert_eq!(s.chars().count(), 144);
                    assert!(s.ends_with('…'));
                } else {
                    panic!("expected OscType::String as arg 0");
                }
            }
            other => panic!("expected OscPacket::Message, got {:?}", other),
        }
    }
}
