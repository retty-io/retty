use super::*;
use crate::channel::channel_test::{MockHandler, Stats};
use crate::channel::Pipeline;
use crate::runtime::mpsc::bounded;
use crate::runtime::mpsc::Sender;
use crate::transport::AsyncTransportTcp;
use anyhow::Result;
use bytes::BytesMut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

#[tokio::test]
async fn async_transport_tcp_test_write_err_on_shutdown() -> Result<()> {
    #[allow(unused_mut)]
    let (tx, mut rx) = bounded(1);
    let sock = MockAsyncTransportWrite::new(tx);

    let pipeline: Pipeline<BytesMut, BytesMut> = Pipeline::new();
    pipeline
        .add_back(AsyncTransportTcp::new(Box::new(sock)))
        .await;
    pipeline.finalize().await;

    let expected = "TESTING MESSAGE".to_string();
    pipeline.write(BytesMut::from(expected.as_bytes())).await;

    let actual = rx.recv().await?;
    assert_eq!(actual, expected.as_bytes());

    // close() the pipeline multiple times.
    pipeline.close().await;
    pipeline.close().await;

    pipeline.write(BytesMut::from(expected.as_bytes())).await;

    let result = rx.recv().await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn async_transport_tcp_test_transport_active_inactive() -> Result<()> {
    let (tx, _rx) = bounded(1);
    let sock = MockAsyncTransportWrite::new(tx);
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let handler: MockHandler<BytesMut, BytesMut> = MockHandler::new(
        "MockHandler",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    );
    let pipeline: Pipeline<BytesMut, BytesMut> = Pipeline::new();
    pipeline
        .add_back(AsyncTransportTcp::new(Box::new(sock)))
        .await;
    pipeline.add_back(handler).await;
    pipeline.finalize().await;

    pipeline.transport_active().await;
    assert_eq!(1, active.load(Ordering::SeqCst));
    pipeline.transport_inactive().await;
    assert_eq!(1, inactive.load(Ordering::SeqCst));
    pipeline.transport_active().await;
    assert_eq!(2, active.load(Ordering::SeqCst));

    Ok(())
}

#[tokio::test]
async fn async_transport_udp_test_write_err_on_shutdown() -> Result<()> {
    #[allow(unused_mut)]
    let (tx, mut rx) = bounded(1);
    let sock = MockAsyncTransportWrite::new(tx);
    let transport = TransportContext {
        local_addr: sock.local_addr().unwrap(),
        peer_addr: Some(sock.peer_addr().unwrap()),
    };

    let pipeline: Pipeline<TaggedBytesMut, TaggedBytesMut> = Pipeline::new();
    pipeline
        .add_back(AsyncTransportUdp::new(Box::new(sock)))
        .await;
    pipeline.finalize().await;

    let expected = "TESTING MESSAGE".to_string();
    pipeline
        .write(TaggedBytesMut {
            transport,
            message: BytesMut::from(expected.as_bytes()),
        })
        .await;

    let actual = rx.recv().await?;
    assert_eq!(actual, expected.as_bytes());

    // close() the pipeline multiple times.
    pipeline.close().await;
    pipeline.close().await;

    pipeline
        .write(TaggedBytesMut {
            transport,
            message: BytesMut::from(expected.as_bytes()),
        })
        .await;

    let result = rx.recv().await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn async_transport_udp_test_transport_active_inactive() -> Result<()> {
    let (tx, _rx) = bounded(1);
    let sock = MockAsyncTransportWrite::new(tx);
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let handler: MockHandler<TaggedBytesMut, TaggedBytesMut> = MockHandler::new(
        "MockHandler",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    );
    let pipeline: Pipeline<TaggedBytesMut, TaggedBytesMut> = Pipeline::new();
    pipeline
        .add_back(AsyncTransportUdp::new(Box::new(sock)))
        .await;
    pipeline.add_back(handler).await;
    pipeline.finalize().await;

    pipeline.transport_active().await;
    assert_eq!(1, active.load(Ordering::SeqCst));
    pipeline.transport_inactive().await;
    assert_eq!(1, inactive.load(Ordering::SeqCst));
    pipeline.transport_active().await;
    assert_eq!(2, active.load(Ordering::SeqCst));

    Ok(())
}
