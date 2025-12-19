use crate::{Capabilities, RecvMeta, Transmit};
use std::{
    fmt::Debug,
    io::{self, IoSliceMut},
    net::SocketAddr,
    task::{Context, Poll},
};

mod tokio;

pub use self::tokio::UdpSocket;

/// Abstract implementation of a UDP socket for runtime independence
pub trait AsyncUdpSocket: Send + Debug + 'static {
    /// Send UDP datagrams from `transmits`, or register to be woken if sending may succeed in the
    /// future
    fn poll_send(
        &self,
        cx: &mut Context<'_>,
        capabilities: &Capabilities,
        transmits: &[Transmit],
    ) -> Poll<Result<usize, io::Error>>;

    /// Receive UDP datagrams, or register to be woken if receiving may succeed in the future
    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>>;

    /// Look up the local IP address and port used by this socket
    fn local_addr(&self) -> io::Result<SocketAddr>;

    /// Look up the peer IP address and port used by this socket
    fn peer_addr(&self) -> io::Result<SocketAddr>;
}
