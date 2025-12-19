use crate::runtime::AsyncUdpSocket;
use crate::{Capabilities, RecvMeta, Transmit, UdpSocketState};
use std::{
    future::poll_fn,
    io,
    net::SocketAddr,
    task::{Context, Poll},
};
use tokio::{io::Interest, net::ToSocketAddrs};

#[derive(Debug)]
pub struct UdpSocket {
    io: tokio::net::UdpSocket,
    inner: UdpSocketState,
}

impl AsyncUdpSocket for UdpSocket {
    fn poll_send(
        &self,
        cx: &mut Context<'_>,
        capabilities: &Capabilities,
        transmits: &[Transmit],
    ) -> Poll<io::Result<usize>> {
        let inner = &self.inner;
        let io = &self.io;
        loop {
            ready!(io.poll_send_ready(cx))?;
            if let Ok(res) = io.try_io(Interest::WRITABLE, || {
                inner.send(io.into(), capabilities, transmits)
            }) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        loop {
            ready!(self.io.poll_recv_ready(cx))?;
            if let Ok(res) = self.io.try_io(Interest::READABLE, || {
                self.inner.recv((&self.io).into(), bufs, meta)
            }) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.peer_addr()
    }
}

impl UdpSocket {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let socket = tokio::net::UdpSocket::bind(addr).await?;
        UdpSocketState::configure((&socket).into())?;
        Ok(Self {
            io: socket,
            inner: UdpSocketState::new(),
        })
    }

    pub async fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        self.io.connect(addr).await
    }

    pub async fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], target: A) -> io::Result<usize> {
        self.io.send_to(buf, target).await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.io.recv_from(buf).await
    }

    pub async fn send(
        &self,
        capabilities: &Capabilities,
        transmits: &[Transmit],
    ) -> io::Result<usize> {
        poll_fn(|cx| self.poll_send(cx, capabilities, transmits)).await
    }

    pub async fn recv(
        &self,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> io::Result<usize> {
        poll_fn(|cx| self.poll_recv(cx, bufs, meta)).await
    }
}
