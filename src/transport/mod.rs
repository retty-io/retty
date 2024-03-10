//! Transport abstraction for TCP and UDP
use bytes::BytesMut;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Instant;

pub use ::async_transport::EcnCodepoint;

/// Type of protocol, either UDP or TCP
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Protocol {
    /// UDP
    #[default]
    UDP,
    /// TCP
    TCP,
}

/// Transport Context with local address, peer address, ECN, protocol, etc.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransportContext {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Peer socket address, either IPv4 or IPv6
    pub peer_addr: SocketAddr,
    /// Type of protocol, either UDP or TCP
    pub protocol: Protocol,
    /// Explicit congestion notification bits to set on the packet
    pub ecn: Option<EcnCodepoint>,
}

impl Default for TransportContext {
    fn default() -> Self {
        Self {
            local_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            peer_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            protocol: Protocol::UDP,
            ecn: None,
        }
    }
}

/// A generic transmit with [TransportContext]
pub struct Transmit<T> {
    /// Received/Sent time
    pub now: Instant,
    /// A transport context with [local_addr](TransportContext::local_addr) and [peer_addr](TransportContext::peer_addr)
    pub transport: TransportContext,
    /// Message body with generic type
    pub message: T,
}

/// BytesMut type transmit with [TransportContext]
pub type TaggedBytesMut = Transmit<BytesMut>;

/// String type transmit with [TransportContext]
pub type TaggedString = Transmit<String>;

/// Four Tuple consists of local address and peer address
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FourTuple {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Peer socket address, either IPv4 or IPv6
    pub peer_addr: SocketAddr,
}

impl From<&TransportContext> for FourTuple {
    fn from(value: &TransportContext) -> Self {
        Self {
            local_addr: value.local_addr,
            peer_addr: value.peer_addr,
        }
    }
}

impl From<TransportContext> for FourTuple {
    fn from(value: TransportContext) -> Self {
        Self {
            local_addr: value.local_addr,
            peer_addr: value.peer_addr,
        }
    }
}

/// Five Tuple consists of local address, peer address and protocol
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FiveTuple {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Peer socket address, either IPv4 or IPv6
    pub peer_addr: SocketAddr,
    /// Type of protocol, either UDP or TCP
    pub protocol: Protocol,
}

impl From<&TransportContext> for FiveTuple {
    fn from(value: &TransportContext) -> Self {
        Self {
            local_addr: value.local_addr,
            peer_addr: value.peer_addr,
            protocol: value.protocol,
        }
    }
}

impl From<TransportContext> for FiveTuple {
    fn from(value: TransportContext) -> Self {
        Self {
            local_addr: value.local_addr,
            peer_addr: value.peer_addr,
            protocol: value.protocol,
        }
    }
}
