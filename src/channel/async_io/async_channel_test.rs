use crate::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use crate::codec::byte_to_message_decoder::{
    ByteToMessageCodec, LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType,
};
use crate::codec::string_codec::{StringCodec, TaggedString, TaggedStringCodec};
use crate::runtime::mpsc::bounded;
use crate::transport::async_io::async_transport_test::MockAsyncTransportWrite;
use crate::transport::{AsyncTransportTcp, AsyncTransportUdp, TaggedBytesMut};
use std::error::Error;
use std::io::ErrorKind;

use anyhow::Result;
use async_trait::async_trait;
use bytes::BytesMut;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Default, Clone)]
pub(crate) struct Stats {
    pub(crate) active: Option<Arc<AtomicUsize>>,
    pub(crate) inactive: Option<Arc<AtomicUsize>>,
    pub(crate) read: Option<Arc<AtomicUsize>>,
    pub(crate) read_exception: Option<Arc<AtomicUsize>>,
    pub(crate) read_eof: Option<Arc<AtomicUsize>>,
    pub(crate) read_timeout: Option<Arc<AtomicUsize>>,
    pub(crate) poll_timeout: Option<Arc<AtomicUsize>>,
    pub(crate) write: Option<Arc<AtomicUsize>>,
    pub(crate) write_exception: Option<Arc<AtomicUsize>>,
    pub(crate) close: Option<Arc<AtomicUsize>>,
}

struct MockDecoder<Rin, Rout> {
    name: String,
    stats: Stats,

    phantom_in: PhantomData<Rin>,
    phantom_out: PhantomData<Rout>,
}
struct MockEncoder<Win, Wout> {
    stats: Stats,

    phantom_in: PhantomData<Win>,
    phantom_out: PhantomData<Wout>,
}
pub(crate) struct MockHandler<R, W> {
    decoder: MockDecoder<R, W>,
    encoder: MockEncoder<W, R>,
}

impl<R, W> MockHandler<R, W> {
    pub fn new(name: &str, stats: Stats) -> Self {
        MockHandler {
            decoder: MockDecoder {
                name: name.to_string(),
                stats: stats.clone(),

                phantom_in: PhantomData,
                phantom_out: PhantomData,
            },
            encoder: MockEncoder {
                stats,
                phantom_in: PhantomData,
                phantom_out: PhantomData,
            },
        }
    }
}

#[async_trait]
impl<Rin: Default + Send + Sync + 'static, Rout: Default + Send + Sync + 'static> InboundHandler
    for MockDecoder<Rin, Rout>
{
    type Rin = Rin;
    type Rout = Rout;

    async fn transport_active(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        if let Some(active) = &self.stats.active {
            active.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_transport_active().await;
    }
    async fn transport_inactive(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        if let Some(inactive) = &self.stats.inactive {
            inactive.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_transport_inactive().await;
    }

    async fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, _msg: Self::Rin) {
        if let Some(read) = &self.stats.read {
            read.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read(Rout::default()).await;
    }
    async fn read_exception(
        &mut self,
        ctx: &InboundContext<Self::Rin, Self::Rout>,
        err: Box<dyn Error + Send + Sync>,
    ) {
        if let Some(read_exception) = &self.stats.read_exception {
            read_exception.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read_exception(err).await;
    }
    async fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        if let Some(read_eof) = &self.stats.read_eof {
            read_eof.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read_eof().await;
    }

    async fn read_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        if let Some(read_timeout) = &self.stats.read_timeout {
            read_timeout.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read_timeout(now).await;
    }
    async fn poll_timeout(
        &mut self,
        ctx: &InboundContext<Self::Rin, Self::Rout>,
        eto: &mut Instant,
    ) {
        if let Some(poll_timeout) = &self.stats.poll_timeout {
            poll_timeout.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_poll_timeout(eto).await;
    }
}

#[async_trait]
impl<Win: Default + Send + Sync + 'static, Wout: Default + Send + Sync + 'static> OutboundHandler
    for MockEncoder<Win, Wout>
{
    type Win = Win;
    type Wout = Wout;

    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, _msg: Self::Win) {
        if let Some(write) = &self.stats.write {
            write.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_write(Wout::default()).await;
    }
    async fn write_exception(
        &mut self,
        ctx: &OutboundContext<Self::Win, Self::Wout>,
        err: Box<dyn Error + Send + Sync>,
    ) {
        if let Some(write_exception) = &self.stats.write_exception {
            write_exception.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_write_exception(err).await;
    }
    async fn close(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>) {
        if let Some(close) = &self.stats.close {
            close.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_close().await;
    }
}

impl<R: Default + Send + Sync + 'static, W: Default + Send + Sync + 'static> Handler
    for MockHandler<R, W>
{
    type Rin = R;
    type Rout = W;
    type Win = W;
    type Wout = R;

    fn name(&self) -> &str {
        self.decoder.name.as_str()
    }

    fn split(
        self,
    ) -> (
        Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
        Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
    ) {
        (Box::new(self.decoder), Box::new(self.encoder))
    }
}

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

    assert_eq!(3, pipeline.len().await);

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

    assert_eq!(3, pipeline.len().await);

    Ok(())
}

#[tokio::test]
async fn pipeline_test_fire_actions() -> Result<()> {
    let stats = Stats {
        active: Some(Arc::new(AtomicUsize::new(0))),
        inactive: Some(Arc::new(AtomicUsize::new(0))),
        read: Some(Arc::new(AtomicUsize::new(0))),
        read_exception: Some(Arc::new(AtomicUsize::new(0))),
        read_eof: Some(Arc::new(AtomicUsize::new(0))),
        read_timeout: Some(Arc::new(AtomicUsize::new(0))),
        poll_timeout: Some(Arc::new(AtomicUsize::new(0))),
        write: Some(Arc::new(AtomicUsize::new(0))),
        write_exception: Some(Arc::new(AtomicUsize::new(0))),
        close: Some(Arc::new(AtomicUsize::new(0))),
    };

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            stats.clone(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            stats.clone(),
        ))
        .await
        .finalize()
        .await;

    pipeline.read("TESTING".to_string()).await;
    assert_eq!(2, stats.read.as_ref().unwrap().load(Ordering::SeqCst));

    pipeline
        .read_exception(Box::new(std::io::Error::new(
            ErrorKind::NotFound,
            "TESTING ERROR",
        )))
        .await;
    assert_eq!(
        2,
        stats
            .read_exception
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst)
    );

    pipeline.read_eof().await;
    assert_eq!(2, stats.read_eof.as_ref().unwrap().load(Ordering::SeqCst));

    pipeline.read_timeout(Instant::now()).await;
    assert_eq!(
        2,
        stats.read_timeout.as_ref().unwrap().load(Ordering::SeqCst)
    );

    pipeline.poll_timeout(&mut Instant::now()).await;
    assert_eq!(
        2,
        stats.poll_timeout.as_ref().unwrap().load(Ordering::SeqCst)
    );

    pipeline.write("TESTING".to_string()).await;
    assert_eq!(2, stats.write.as_ref().unwrap().load(Ordering::SeqCst));

    pipeline
        .write_exception(Box::new(std::io::Error::new(
            ErrorKind::NotFound,
            "TESTING ERROR",
        )))
        .await;
    assert_eq!(
        2,
        stats
            .write_exception
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst)
    );

    pipeline.close().await;
    assert_eq!(2, stats.close.as_ref().unwrap().load(Ordering::SeqCst));

    Ok(())
}
#[tokio::test]
async fn pipeline_test_dynamic_construction() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))
        .await;

    // Exercise both addFront and addBack. Final pipeline is
    // StI <-> ItS <-> StS <-> StS <-> StI <-> ItS
    pipeline
        .add_front(MockHandler::<usize, String>::new(
            "handler3",
            Stats::default(),
        ))
        .await
        .add_front(MockHandler::<String, usize>::new(
            "handler4",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<String, usize>::new(
            "handler5",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<usize, String>::new(
            "handler6",
            Stats::default(),
        ))
        .await
        .finalize()
        .await;

    assert_eq!(6, pipeline.len().await);

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
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))
        .await;

    // Exercise both addFront and addBack. Final pipeline is
    // StI <-> ItS <-> StS <-> StS <-> StI <-> ItS
    pipeline
        .add_front(MockHandler::<usize, String>::new(
            "handler3",
            Stats::default(),
        ))
        .await
        .add_front(MockHandler::<String, usize>::new(
            "handler4",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<String, usize>::new(
            "handler5",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<usize, String>::new(
            "handler6",
            Stats::default(),
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

    assert_eq!(2, pipeline.len().await);

    Ok(())
}

#[tokio::test]
async fn pipeline_test_remove_front_back() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))
        .await
        .add_back(MockHandler::<String, String>::new(
            "handler3",
            Stats::default(),
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

    assert_eq!(1, pipeline.len().await);

    pipeline.read("TESTING INBOUND MESSAGE".to_owned()).await;
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned()).await;

    pipeline.remove("handler2").await?;

    assert_eq!(0, pipeline.len().await);

    Ok(())
}

#[tokio::test]
async fn pipeline_test_num_handlers() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))
        .await;
    assert_eq!(1, pipeline.len().await);

    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))
        .await;
    assert_eq!(2, pipeline.len().await);

    pipeline.finalize().await;
    assert_eq!(2, pipeline.len().await);

    pipeline.remove("handler1").await?;
    assert_eq!(1, pipeline.len().await);

    pipeline.remove("handler2").await?;
    assert_eq!(0, pipeline.len().await);

    Ok(())
}
