use super::*;
use crate::runtime::mpsc::Sender;
use bytes::BytesMut;

pub(crate) struct MockAsyncTransportWrite {
    tx: Sender<BytesMut>,
}

impl MockAsyncTransportWrite {
    pub(crate) fn new(tx: Sender<BytesMut>) -> Self {
        Self { tx }
    }
}

impl TransportAddress for MockAsyncTransportWrite {
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(SocketAddr::from_str("127.0.0.1:1234").unwrap())
    }

    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(SocketAddr::from_str("127.0.0.1:4321").unwrap())
    }
}

#[async_trait]
impl AsyncTransportWrite for MockAsyncTransportWrite {
    async fn write(&mut self, buf: &[u8], _target: Option<SocketAddr>) -> std::io::Result<usize> {
        #[cfg(feature = "runtime-tokio")]
        let _ = self.tx.send(BytesMut::from(buf));

        #[cfg(feature = "runtime-async-std")]
        let _ = self.tx.send(BytesMut::from(buf)).await;
        Ok(buf.len())
    }
}
