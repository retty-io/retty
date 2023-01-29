use crate::channel::Pipeline;
use crate::runtime::mpsc::bounded;
use crate::transport::transport_test::MockAsyncTransportWrite;
use crate::transport::{AsyncTransportUdp, TaggedBytesMut, TransportAddress, TransportContext};
use anyhow::Result;
use bytes::BytesMut;

#[tokio::test]
async fn async_socket_handler_test() -> Result<()> {
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

    Ok(())
}
