//! Asynchronous transport abstraction for TCP and UDP

use async_trait::async_trait;
use async_transport::{AsyncUdpSocket, EcnCodepoint, RecvMeta, Transmit, UdpState};
use bytes::BytesMut;
use std::io::IoSliceMut;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use crate::runtime::io::{AsyncReadExt, AsyncWriteExt};
use crate::runtime::net::{OwnedReadHalf, OwnedWriteHalf, UdpSocket};

#[cfg(not(feature = "sans-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "sans-io"))]
pub use self::async_io::{
    async_transport_tcp::AsyncTransportTcp, async_transport_udp::AsyncTransportUdp,
};

#[cfg(feature = "sans-io")]
pub(crate) mod sans_io;
#[cfg(feature = "sans-io")]
pub use sans_io::AsyncTransport;

/// Transport Context with local address and optional peer address
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransportContext {
    /// Local socket address, either IPv4 or IPv6
    pub local_addr: SocketAddr,
    /// Optional peer socket address, either IPv4 or IPv6
    pub peer_addr: Option<SocketAddr>,
}

impl Default for TransportContext {
    fn default() -> Self {
        Self {
            local_addr: SocketAddr::from_str("0.0.0.0:0").unwrap(),
            peer_addr: None,
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
    /// Explicit congestion notification bits to set on the packet
    pub ecn: Option<EcnCodepoint>,
    /// Message body with [BytesMut](bytes::BytesMut) type
    pub message: BytesMut,
}

impl Default for TaggedBytesMut {
    fn default() -> Self {
        Self {
            now: Instant::now(),
            transport: TransportContext::default(),
            ecn: None,
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

/// Read half of an asynchronous transport
#[async_trait]
pub trait AsyncTransportRead: TransportAddress {
    /// Reads data from an asynchronous transport into the provided buffer.
    /// On success, returns the number of bytes read and the origin.
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)>;

    /// Receives data into the provided buffers and meta data like ECN information.
    async fn recv(
        &mut self,
        _bufs: &mut [IoSliceMut<'_>],
        _meta: &mut [RecvMeta],
    ) -> std::io::Result<usize>;
}

/// Write half of an asynchronous transport
#[async_trait]
pub trait AsyncTransportWrite: TransportAddress {
    /// Sends data to an asynchronous transport to the given address, optional for TCP transport.
    /// On success, returns the number of bytes written.
    async fn write(&mut self, buf: &[u8], target: Option<SocketAddr>) -> std::io::Result<usize>;

    /// Send data from transmits
    async fn send(&mut self, _transmits: &[Transmit]) -> std::io::Result<usize>;
}

///////////////////////////////////////////////////////////////////////////////////////////////////

impl TransportAddress for OwnedReadHalf {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.local_addr()
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.peer_addr()
    }
}

#[cfg(not(feature = "runtime-async-std"))]
impl TransportAddress for OwnedWriteHalf {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.local_addr()
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.peer_addr()
    }
}

#[async_trait]
impl AsyncTransportRead for OwnedReadHalf {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)> {
        let n = AsyncReadExt::read(&mut self, buf).await?;
        Ok((n, None))
    }

    async fn recv(
        &mut self,
        _bufs: &mut [IoSliceMut<'_>],
        _meta: &mut [RecvMeta],
    ) -> std::io::Result<usize> {
        unimplemented!()
    }
}

#[async_trait]
impl AsyncTransportWrite for OwnedWriteHalf {
    async fn write(&mut self, buf: &[u8], _target: Option<SocketAddr>) -> std::io::Result<usize> {
        AsyncWriteExt::write(&mut self, buf).await
    }

    async fn send(&mut self, _transmits: &[Transmit]) -> std::io::Result<usize> {
        unimplemented!()
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
impl TransportAddress for Arc<UdpSocket> {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        UdpSocket::local_addr(self)
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        UdpSocket::peer_addr(self)
    }
}

#[async_trait]
impl AsyncTransportRead for Arc<UdpSocket> {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)> {
        let (len, addr) = self.recv_from(buf).await?;
        Ok((len, Some(addr)))
    }

    async fn recv(
        &mut self,
        _bufs: &mut [IoSliceMut<'_>],
        _meta: &mut [RecvMeta],
    ) -> std::io::Result<usize> {
        unimplemented!()
    }
}

#[async_trait]
impl AsyncTransportWrite for Arc<UdpSocket> {
    async fn write(&mut self, buf: &[u8], target: Option<SocketAddr>) -> std::io::Result<usize> {
        let target = target.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "SocketAddr is required for UdpSocket write".to_string(),
            )
        })?;
        self.send_to(buf, target).await
    }

    async fn send(&mut self, _transmits: &[Transmit]) -> std::io::Result<usize> {
        unimplemented!()
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
impl TransportAddress for Arc<async_transport::UdpSocket> {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        async_transport::UdpSocket::local_addr(self)
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        async_transport::UdpSocket::peer_addr(self)
    }
}

#[async_trait]
impl AsyncTransportRead for Arc<async_transport::UdpSocket> {
    async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)> {
        unimplemented!()
    }

    async fn recv(
        &mut self,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> std::io::Result<usize> {
        async_transport::UdpSocket::recv(self, bufs, meta).await
    }
}

#[async_trait]
impl AsyncTransportWrite for Arc<async_transport::UdpSocket> {
    async fn write(&mut self, _buf: &[u8], _target: Option<SocketAddr>) -> std::io::Result<usize> {
        unimplemented!()
    }

    async fn send(&mut self, transmits: &[Transmit]) -> std::io::Result<usize> {
        let state = UdpState::new();
        async_transport::UdpSocket::send(self, &state, transmits).await
    }
}
