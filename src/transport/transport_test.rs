use crate::channel::{
    channel_test::{MockHandler, Stats},
    *,
};
use crate::transport::*;

use anyhow::Result;
use bytes::BytesMut;
use local_sync::mpsc::unbounded::channel;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn async_transport_test_write_err_on_shutdown() -> Result<()> {
    let (tx, mut rx) = channel::<BytesMut>();
    let writer = AsyncTransportWrite::new(
        tx,
        SocketAddr::from_str("127.0.0.1:1234")?,
        Some(SocketAddr::from_str("127.0.0.1:4321")?),
    );

    let pipeline: pipeline::Pipeline<BytesMut, bytes::BytesMut> = Pipeline::new();
    pipeline.add_back(AsyncTransport::new(writer))?;
    let pipeline = pipeline.finalize()?;

    let expected = "TESTING MESSAGE".to_string();
    pipeline.write(BytesMut::from(expected.as_bytes()));

    let actual = rx.try_recv()?;
    assert_eq!(actual, expected.as_bytes());

    // close() the pipeline multiple times.
    pipeline.close();
    pipeline.close();

    pipeline.write(BytesMut::from(expected.as_bytes()));

    let result = rx.try_recv();
    assert!(result.is_err());

    Ok(())
}

#[test]
fn async_transport_test_transport_active_inactive() -> Result<()> {
    let (tx, _) = channel::<BytesMut>();
    let writer = AsyncTransportWrite::new(
        tx,
        SocketAddr::from_str("127.0.0.1:1234")?,
        Some(SocketAddr::from_str("127.0.0.1:4321")?),
    );
    let active = Rc::new(AtomicUsize::new(0));
    let inactive = Rc::new(AtomicUsize::new(0));

    let handler: MockHandler<BytesMut, BytesMut> = MockHandler::new(
        "MockHandler",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    );
    let pipeline: Pipeline<BytesMut, BytesMut> = Pipeline::new();
    pipeline.add_back(AsyncTransport::new(writer))?;
    pipeline.add_back(handler)?;
    let pipeline = pipeline.finalize()?;

    pipeline.transport_active();
    assert_eq!(1, active.load(Ordering::SeqCst));
    pipeline.transport_inactive();
    assert_eq!(1, inactive.load(Ordering::SeqCst));
    pipeline.transport_active();
    assert_eq!(2, active.load(Ordering::SeqCst));

    Ok(())
}
