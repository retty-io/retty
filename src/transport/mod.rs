//! Asynchronous transport abstraction for TCP and UDP
use bytes::BytesMut;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Instant;

mod async_transport;
pub use async_transport::AsyncTransport;

/// Explicit congestion notification codepoint
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EcnCodepoint {
    #[doc(hidden)]
    Ect0 = 0b10,
    #[doc(hidden)]
    Ect1 = 0b01,
    #[doc(hidden)]
    Ce = 0b11,
}

impl EcnCodepoint {
    /// Create new object from the given bits
    pub fn from_bits(x: u8) -> Option<Self> {
        use self::EcnCodepoint::*;
        Some(match x & 0b11 {
            0b10 => Ect0,
            0b01 => Ect1,
            0b11 => Ce,
            _ => {
                return None;
            }
        })
    }
}

/// Transport Context with local address and optional peer address
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransportContext {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Optional peer socket address, either IPv4 or IPv6
    pub peer_addr: Option<SocketAddr>,
    /// Explicit congestion notification bits to set on the packet
    pub ecn: Option<EcnCodepoint>,
}

impl Default for TransportContext {
    fn default() -> Self {
        Self {
            local_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            peer_addr: None,
            ecn: None,
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

/// Obtains local address and peer address
pub trait TransportAddress {
    /// Returns the local address
    fn local_addr(&self) -> std::io::Result<SocketAddr>;
    /// Returns the peer address
    fn peer_addr(&self) -> std::io::Result<SocketAddr>;
}
