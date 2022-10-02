use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::runtime::io::{AsyncReadExt, AsyncWriteExt};
use crate::runtime::net::{OwnedReadHalf, OwnedWriteHalf, UdpSocket};

pub mod async_transport_tcp;
pub mod async_transport_udp;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransportContext {
    pub local_addr: SocketAddr,
    pub peer_addr: SocketAddr,
}

#[async_trait]
pub trait AsyncTransportAddress {
    fn local_addr(&self) -> std::io::Result<SocketAddr>;
    fn peer_addr(&self) -> std::io::Result<SocketAddr>;
}

#[async_trait]
pub trait AsyncTransportRead: AsyncTransportAddress {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)>;
}

#[async_trait]
pub trait AsyncTransportWrite: AsyncTransportAddress {
    async fn write(&mut self, buf: &[u8], target: Option<SocketAddr>) -> std::io::Result<usize>;
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[async_trait]
impl AsyncTransportAddress for OwnedReadHalf {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.local_addr()
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.peer_addr()
    }
}

#[cfg(not(feature = "runtime-async-std"))]
#[async_trait]
impl AsyncTransportAddress for OwnedWriteHalf {
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
}

#[async_trait]
impl AsyncTransportWrite for OwnedWriteHalf {
    async fn write(&mut self, buf: &[u8], _target: Option<SocketAddr>) -> std::io::Result<usize> {
        AsyncWriteExt::write(&mut self, buf).await
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[async_trait]
impl AsyncTransportAddress for Arc<UdpSocket> {
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
}
