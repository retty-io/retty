use crate::channel::Pipeline;
use crate::runtime::mpsc::bounded;
use crate::transport::transport_test::MockAsyncTransportWrite;
use crate::transport::AsyncTransportTcp;
use anyhow::Result;
use bytes::BytesMut;

#[tokio::test]
async fn async_transport_udp_test_write_err_on_shutdown() -> Result<()> {
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
