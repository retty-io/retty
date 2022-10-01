use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

use crate::transport::{AsyncTransportAddress, AsyncTransportRead, AsyncTransportWrite};

#[async_trait]
impl AsyncTransportAddress for UdpSocket {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.local_addr()
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.peer_addr()
    }
}

#[async_trait]
impl AsyncTransportRead for UdpSocket {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)> {
        let (len, addr) = self.recv_from(buf).await?;
        Ok((len, Some(addr)))
    }
}

#[async_trait]
impl AsyncTransportWrite for UdpSocket {
    async fn write(&mut self, buf: &[u8], target: Option<SocketAddr>) -> std::io::Result<usize> {
        let target = target.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "Address Not Available".to_string(),
            )
        })?;
        self.send_to(buf, target).await
    }
}
