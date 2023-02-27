//! Asynchronous transport abstraction for TCP and UDP

use async_transport::EcnCodepoint;
use bytes::BytesMut;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Instant;

#[cfg(not(feature = "metal-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "metal-io"))]
pub use self::async_io::{
    async_transport_tcp::AsyncTransportTcp, async_transport_udp::AsyncTransportUdp,
    async_transport_udp_ecn::AsyncTransportUdpEcn, AsyncTransportRead, AsyncTransportWrite,
};

#[cfg(feature = "metal-io")]
pub(crate) mod metal_io;
#[cfg(feature = "metal-io")]
pub use metal_io::AsyncTransport;

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
