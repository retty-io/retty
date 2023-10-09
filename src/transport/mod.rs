//! Asynchronous transport abstraction for TCP and UDP
use bytes::BytesMut;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Instant;

mod async_transport;

#[cfg(test)]
mod transport_test;

pub use self::async_transport::AsyncTransport;
pub use ::async_transport::EcnCodepoint;

/// Re-export local_sync::mpsc::unbounded::Tx as LocalSender
pub use local_sync::mpsc::unbounded::Tx as LocalSender;

/// Transport Context with local address and optional peer address
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransportContext {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Peer socket address, either IPv4 or IPv6
    pub peer_addr: SocketAddr,
    /// Explicit congestion notification bits to set on the packet
    pub ecn: Option<EcnCodepoint>,
}

impl Default for TransportContext {
    fn default() -> Self {
        Self {
            local_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            peer_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            ecn: None,
        }
    }
}

/// A tagged [BytesMut] with [TransportContext]
#[derive(Clone)]
pub struct TaggedBytesMut {
    /// Received/Sent time
    pub now: Instant,
    /// A transport context with [local_addr](TransportContext::local_addr) and [peer_addr](TransportContext::peer_addr)
    pub transport: TransportContext,
    /// Message body with [BytesMut] type
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

/// Write half of an asynchronous transport
#[derive(Clone)]
pub struct AsyncTransportWrite<R> {
    sender: LocalSender<R>,
    local_addr: SocketAddr,
    peer_addr: Option<SocketAddr>,
}

impl<R> AsyncTransportWrite<R> {
    /// Creates a new AsyncTransportWrite
    pub fn new(
        sender: LocalSender<R>,
        local_addr: SocketAddr,
        peer_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            sender,
            local_addr,
            peer_addr,
        }
    }

    /// Returns local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns peer address
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.peer_addr
    }

    /// Writes a message
    pub fn write(&self, msg: R) -> Result<(), std::io::Error> {
        self.sender.send(msg).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, format!("{:?}", err))
        })
    }
}
