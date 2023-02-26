use async_trait::async_trait;
use async_transport::{AsyncUdpSocket, Capabilities, RecvMeta, Transmit};
use std::io::IoSliceMut;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::runtime::io::{AsyncReadExt, AsyncWriteExt};
use crate::runtime::net::{OwnedReadHalf, OwnedWriteHalf, UdpSocket};
use crate::transport::TransportAddress;

#[cfg(test)]
pub(crate) mod async_transport_test;

pub(crate) mod async_transport_tcp;
pub(crate) mod async_transport_udp;
pub(crate) mod async_transport_udp_ecn;

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
    async fn send(
        &mut self,
        _capabilities: &Capabilities,
        _transmits: &[Transmit],
    ) -> std::io::Result<usize>;
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
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            String::from("Unsupported recv"),
        ))
    }
}

#[async_trait]
impl AsyncTransportWrite for OwnedWriteHalf {
    async fn write(&mut self, buf: &[u8], _target: Option<SocketAddr>) -> std::io::Result<usize> {
        AsyncWriteExt::write(&mut self, buf).await
    }

    async fn send(
        &mut self,
        _capabilities: &Capabilities,
        _transmits: &[Transmit],
    ) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            String::from("Unsupported send"),
        ))
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
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            String::from("Unsupported recv"),
        ))
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

    async fn send(
        &mut self,
        _capabilities: &Capabilities,
        _transmits: &[Transmit],
    ) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            String::from("Unsupported send"),
        ))
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
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)> {
        let (len, addr) = self.recv_from(buf).await?;
        Ok((len, Some(addr)))
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
    async fn write(&mut self, buf: &[u8], target: Option<SocketAddr>) -> std::io::Result<usize> {
        let target = target.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "SocketAddr is required for UdpSocket write".to_string(),
            )
        })?;
        self.send_to(buf, target).await
    }

    async fn send(
        &mut self,
        capabilities: &Capabilities,
        transmits: &[Transmit],
    ) -> std::io::Result<usize> {
        async_transport::UdpSocket::send(self, capabilities, transmits).await
    }
}
