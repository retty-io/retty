use crate::channel::Pipeline;
use crate::runtime::mpsc::bounded;
use crate::transport::transport_test::{MockAsyncTransportWrite, MockHandler};
use crate::transport::{AsyncTransportUdp, TaggedBytesMut, TransportAddress, TransportContext};
use anyhow::Result;
use bytes::BytesMut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

    let handler: MockHandler<TaggedBytesMut, TaggedBytesMut> =
        MockHandler::new(active.clone(), inactive.clone());
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