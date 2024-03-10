//! Asynchronous transport abstraction for TCP and UDP
use bytes::BytesMut;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Instant;

pub use ::async_transport::EcnCodepoint;

/// Protocol type: UDP or TCP
#[derive(Default, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Protocol {
    /// UDP
    #[default]
    UDP,
    /// TCP
    TCP,
}

/// Transport Context with local address and optional peer address
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransportContext {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Peer socket address, either IPv4 or IPv6
    pub peer_addr: SocketAddr,
    /// Explicit congestion notification bits to set on the packet
    pub ecn: Option<EcnCodepoint>,
    /// Type of protocol
    pub protocol: Protocol,
}

impl Default for TransportContext {
    fn default() -> Self {
        Self {
            local_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            peer_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            ecn: None,
            protocol: Protocol::UDP,
        }
    }
}

/// A tagged [BytesMut](bytes::BytesMut) with [TransportContext]
#[derive(Clone)]
pub struct TaggedBytesMut {
    /// Received/Sent time
    pub now: Instant,
    /// A transport context with [local_addr](TransportContext::local_addr) and [peer_addr](TransportContext::peer_addr)
    pub transport: TransportContext,
    /// Message body with [BytesMut](bytes::BytesMut) type
    pub message: BytesMut,
}

impl Default for TaggedBytesMut {
    fn default() -> Self {
        Self {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: BytesMut::default(),
        }
    }
}
