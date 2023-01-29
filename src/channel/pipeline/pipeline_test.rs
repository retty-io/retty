use crate::channel::Pipeline;
use crate::codec::byte_to_message_decoder::{
    ByteToMessageCodec, LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType,
};
use crate::codec::string_codec::{StringCodec, TaggedString, TaggedStringCodec};
use crate::runtime::mpsc::bounded;
use crate::transport::transport_test::{MockAsyncTransportWrite, MockHandler};
use crate::transport::{AsyncTransportTcp, AsyncTransportUdp, TaggedBytesMut};

use anyhow::Result;
use bytes::BytesMut;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[tokio::test]
async fn pipeline_test_real_handlers_compile_tcp() -> Result<()> {
    let (tx, _rx) = bounded(1);
    let sock = Box::new(MockAsyncTransportWrite::new(tx));

    let pipeline: Pipeline<BytesMut, String> = Pipeline::new();

    let async_transport_handler = AsyncTransportTcp::new(sock);
    let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
    ));
    let string_codec_handler = StringCodec::new();

    pipeline
        .add_back(async_transport_handler)
        .await
        .add_back(line_based_frame_decoder_handler)
        .await
        .add_back(string_codec_handler)
        .await
        .finalize()
        .await;

    Ok(())
}

#[tokio::test]
async fn pipeline_test_real_handlers_compile_udp() -> Result<()> {
    let (tx, _rx) = bounded(1);
    let sock = Box::new(MockAsyncTransportWrite::new(tx));

    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

    let async_transport_handler = AsyncTransportUdp::new(sock);
    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
    ));
    let string_codec_handler = TaggedStringCodec::new();

    pipeline
        .add_back(async_transport_handler)
        .await
        .add_back(line_based_frame_decoder_handler)
        .await
        .add_back(string_codec_handler)
        .await
        .finalize()
        .await;

    Ok(())
}

#[tokio::test]
async fn pipeline_test_dynamic_construction() -> Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await;

    // Exercise both addFront and addBack. Final pipeline is
    // StI <-> ItS <-> StS <-> StS <-> StI <-> ItS
    pipeline
        .add_front(MockHandler::<usize, String>::new(
            "handler3".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_front(MockHandler::<String, usize>::new(
            "handler4".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<String, usize>::new(
            "handler5".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<usize, String>::new(
            "handler6".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .finalize()
        .await;

    pipeline.read("TESTING INBOUND MESSAGE".to_owned()).await;
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned()).await;

    Ok(())
}

/*TODO: fix panic backtrace
#[tokio::test]
async fn pipeline_test_dynamic_construction_read_fail() -> Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            active.clone(),
            inactive.clone(),
        ))
        .await;
    pipeline
        .add_back(MockHandler::<String, String>::new(
            active.clone(),
            inactive.clone(),
        ))
        .await;

    pipeline
        .add_front(MockHandler::<String, usize>::new(
            active.clone(),
            inactive.clone(),
        ))
        .await;

    pipeline.finalize().await;

    // Wrap it all in a std::sync::Mutex
    let pipeline = Arc::new(std::sync::Mutex::new(pipeline));

    let result = std::panic::catch_unwind(|| {
        let pipeline = pipeline.lock().unwrap();

        // Enter the runtime
        let handle = tokio::runtime::Handle::current();

        let _ = handle.enter();

        futures::executor::block_on(pipeline.read("TESTING INBOUND MESSAGE".to_owned()));
    });
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn pipeline_test_dynamic_construction_write_fail() -> Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            active.clone(),
            inactive.clone(),
        ))
        .await;
    pipeline
        .add_back(MockHandler::<String, String>::new(
            active.clone(),
            inactive.clone(),
        ))
        .await;

    pipeline
        .add_front(MockHandler::<String, usize>::new(
            active.clone(),
            inactive.clone(),
        ))
        .await;

    pipeline.finalize().await;

    // Wrap it all in a std::sync::Mutex
    let pipeline = Arc::new(std::sync::Mutex::new(pipeline));

    let result = std::panic::catch_unwind(|| {
        let pipeline = pipeline.lock().unwrap();

        // Enter the runtime
        let handle = tokio::runtime::Handle::current();

        let _ = handle.enter();

        futures::executor::block_on(pipeline.write("TESTING OUTBOUND MESSAGE".to_owned()));
    });
    assert!(result.is_err());

    Ok(())
}*/

#[tokio::test]
async fn pipeline_test_remove_handler() -> Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await;

    // Exercise both addFront and addBack. Final pipeline is
    // StI <-> ItS <-> StS <-> StS <-> StI <-> ItS
    pipeline
        .add_front(MockHandler::<usize, String>::new(
            "handler3".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_front(MockHandler::<String, usize>::new(
            "handler4".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<String, usize>::new(
            "handler5".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<usize, String>::new(
            "handler6".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .finalize()
        .await;

    pipeline.remove("handler3").await?;
    pipeline.remove("handler4").await?;
    pipeline.remove("handler5").await?;
    pipeline.remove("handler6").await?;
    pipeline.finalize().await;

    pipeline.read("TESTING INBOUND MESSAGE".to_owned()).await;
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned()).await;

    Ok(())
}

#[tokio::test]
async fn pipeline_test_remove_front_back() -> Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    let inactive = Arc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler3".to_string(),
            active.clone(),
            inactive.clone(),
        ))
        .await
        .finalize()
        .await;

    pipeline
        .remove_front()
        .await?
        .remove_back()
        .await?
        .finalize()
        .await;

    pipeline.read("TESTING INBOUND MESSAGE".to_owned()).await;
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned()).await;

    pipeline.remove("handler2").await?;

    Ok(())
}
