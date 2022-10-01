use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;

pub mod tcp;
pub mod udp;

#[async_trait]
pub trait AsyncTransportAddress {
    fn local_addr(&self) -> std::io::Result<SocketAddr>;
    fn peer_addr(&self) -> std::io::Result<SocketAddr>;
}

#[async_trait]
pub trait AsyncTransportRead {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, Option<SocketAddr>)>;
}

#[async_trait]
pub trait AsyncTransportWrite {
    async fn write<A: ToSocketAddrs + std::marker::Send>(
        &mut self,
        buf: &[u8],
        target: Option<A>,
    ) -> std::io::Result<usize>;
}
