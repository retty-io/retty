use crate::channel::*;
use crate::codec::byte_to_message_decoder::{
    LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType,
};
use crate::codec::string_codec::{TaggedString, TaggedStringCodec};
use crate::transport::{AsyncTransport, AsyncTransportWrite, TaggedBytesMut};
use std::any::Any;

use anyhow::Result;
use local_sync::mpsc::unbounded::channel;
use std::error::Error;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Default, Clone)]
pub(crate) struct Stats {
    pub(crate) active: Option<Rc<AtomicUsize>>,
    pub(crate) inactive: Option<Rc<AtomicUsize>>,
    pub(crate) read: Option<Rc<AtomicUsize>>,
    pub(crate) read_exception: Option<Rc<AtomicUsize>>,
    pub(crate) read_eof: Option<Rc<AtomicUsize>>,
    pub(crate) read_timeout: Option<Rc<AtomicUsize>>,
    pub(crate) poll_timeout: Option<Rc<AtomicUsize>>,
    pub(crate) write: Option<Rc<AtomicUsize>>,
    pub(crate) write_exception: Option<Rc<AtomicUsize>>,
    pub(crate) close: Option<Rc<AtomicUsize>>,
}

struct MockDecoder<Rin, Rout> {
    name: String,
    stats: Stats,

    phantom_in: PhantomData<Rin>,
    phantom_out: PhantomData<Rout>,
}
struct MockEncoder<Win, Wout> {
    name: String,
    stats: Stats,

    phantom_in: PhantomData<Win>,
    phantom_out: PhantomData<Wout>,
}
pub(crate) struct MockHandler<R, W> {
    name: String,
    decoder: MockDecoder<R, W>,
    encoder: MockEncoder<W, R>,
}

impl<R, W> MockHandler<R, W> {
    pub fn new(name: &str, stats: Stats) -> Self {
        MockHandler {
            name: name.to_string(),
            decoder: MockDecoder {
                name: format!("{} decoder", name),
                stats: stats.clone(),

                phantom_in: PhantomData,
                phantom_out: PhantomData,
            },
            encoder: MockEncoder {
                name: format!("{} encoder", name),
                stats,

                phantom_in: PhantomData,
                phantom_out: PhantomData,
            },
        }
    }
}

impl<Rin: Default + 'static, Rout: Default + 'static> InboundHandler for MockDecoder<Rin, Rout> {
    type Rin = Rin;
    type Rout = Rout;

    fn transport_active(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        if let Some(active) = &self.stats.active {
            active.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_transport_active();
    }
    fn transport_inactive(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        if let Some(inactive) = &self.stats.inactive {
            inactive.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_transport_inactive();
    }

    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, _msg: Self::Rin) {
        if let Some(read) = &self.stats.read {
            read.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read(Rout::default());
    }
    fn read_exception(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, err: Box<dyn Error>) {
        if let Some(read_exception) = &self.stats.read_exception {
            read_exception.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read_exception(err);
    }
    fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        if let Some(read_eof) = &self.stats.read_eof {
            read_eof.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_read_eof();
    }

    fn handle_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        if let Some(read_timeout) = &self.stats.read_timeout {
            read_timeout.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_handle_timeout(now);
    }
    fn poll_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, eto: &mut Instant) {
        if let Some(poll_timeout) = &self.stats.poll_timeout {
            poll_timeout.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_poll_timeout(eto);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<Win: Default + 'static, Wout: Default + 'static> OutboundHandler for MockEncoder<Win, Wout> {
    type Win = Win;
    type Wout = Wout;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, _msg: Self::Win) {
        if let Some(write) = &self.stats.write {
            write.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_write(Wout::default());
    }
    fn write_exception(
        &mut self,
        ctx: &OutboundContext<Self::Win, Self::Wout>,
        err: Box<dyn Error>,
    ) {
        if let Some(write_exception) = &self.stats.write_exception {
            write_exception.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_write_exception(err);
    }
    fn close(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>) {
        if let Some(close) = &self.stats.close {
            close.fetch_add(1, Ordering::SeqCst);
        }
        ctx.fire_close();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<R: Default + 'static, W: Default + 'static> Handler for MockHandler<R, W> {
    type Rin = R;
    type Rout = W;
    type Win = W;
    type Wout = R;

    fn name(&self) -> &str {
        self.name.as_str()
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

#[test]
fn pipeline_test_real_handlers_compile_tcp() -> Result<()> {
    let (tx, _rx) = channel::<TaggedBytesMut>();
    let writer = AsyncTransportWrite::new(
        tx,
        SocketAddr::from_str("127.0.0.1:1234")?,
        Some(SocketAddr::from_str("127.0.0.1:4321")?),
    );

    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

    let async_transport_handler = AsyncTransport::new(writer);
    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
    ));
    let string_codec_handler = TaggedStringCodec::new();

    pipeline
        .add_back(async_transport_handler)?
        .add_back(line_based_frame_decoder_handler)?
        .add_back(string_codec_handler)?;
    let pipeline = pipeline.finalize()?;

    assert_eq!(3, pipeline.len());

    Ok(())
}

#[test]
fn pipeline_test_real_handlers_compile_udp() -> Result<()> {
    let (tx, _rx) = channel::<TaggedBytesMut>();
    let writer = AsyncTransportWrite::new(
        tx,
        SocketAddr::from_str("127.0.0.1:1234")?,
        Some(SocketAddr::from_str("127.0.0.1:4321")?),
    );

    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

    let async_transport_handler = AsyncTransport::new(writer);
    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
    ));
    let string_codec_handler = TaggedStringCodec::new();

    pipeline
        .add_back(async_transport_handler)?
        .add_back(line_based_frame_decoder_handler)?
        .add_back(string_codec_handler)?;
    let pipeline = pipeline.finalize()?;

    assert_eq!(3, pipeline.len());

    Ok(())
}

#[test]
fn pipeline_test_fire_actions() -> Result<()> {
    let stats = Stats {
        active: Some(Rc::new(AtomicUsize::new(0))),
        inactive: Some(Rc::new(AtomicUsize::new(0))),
        read: Some(Rc::new(AtomicUsize::new(0))),
        read_exception: Some(Rc::new(AtomicUsize::new(0))),
        read_eof: Some(Rc::new(AtomicUsize::new(0))),
        read_timeout: Some(Rc::new(AtomicUsize::new(0))),
        poll_timeout: Some(Rc::new(AtomicUsize::new(0))),
        write: Some(Rc::new(AtomicUsize::new(0))),
        write_exception: Some(Rc::new(AtomicUsize::new(0))),
        close: Some(Rc::new(AtomicUsize::new(0))),
    };

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            stats.clone(),
        ))?
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            stats.clone(),
        ))?;
    let pipeline = pipeline.finalize()?;

    pipeline.read("TESTING".to_string());
    assert_eq!(2, stats.read.as_ref().unwrap().load(Ordering::SeqCst));

    pipeline.read_exception(Box::new(std::io::Error::new(
        ErrorKind::NotFound,
        "TESTING ERROR",
    )));
    assert_eq!(
        2,
        stats
            .read_exception
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst)
    );

    pipeline.read_eof();
    assert_eq!(2, stats.read_eof.as_ref().unwrap().load(Ordering::SeqCst));

    pipeline.handle_timeout(Instant::now());
    assert_eq!(
        2,
        stats.read_timeout.as_ref().unwrap().load(Ordering::SeqCst)
    );

    pipeline.poll_timeout(&mut Instant::now());
    assert_eq!(
        2,
        stats.poll_timeout.as_ref().unwrap().load(Ordering::SeqCst)
    );

    pipeline.write("TESTING".to_string());
    assert_eq!(2, stats.write.as_ref().unwrap().load(Ordering::SeqCst));

    pipeline.write_exception(Box::new(std::io::Error::new(
        ErrorKind::NotFound,
        "TESTING ERROR",
    )));
    assert_eq!(
        2,
        stats
            .write_exception
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst)
    );

    pipeline.close();
    assert_eq!(2, stats.close.as_ref().unwrap().load(Ordering::SeqCst));

    Ok(())
}

#[test]
fn pipeline_test_dynamic_construction() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))?;

    // Exercise both addFront and addBack. Final pipeline is
    // StI <-> ItS <-> StS <-> StS <-> StI <-> ItS
    pipeline
        .add_front(MockHandler::<usize, String>::new(
            "handler3",
            Stats::default(),
        ))?
        .add_front(MockHandler::<String, usize>::new(
            "handler4",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, usize>::new(
            "handler5",
            Stats::default(),
        ))?
        .add_back(MockHandler::<usize, String>::new(
            "handler6",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;

    assert_eq!(6, pipeline.len());

    pipeline.read("TESTING INBOUND MESSAGE".to_owned());
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned());

    Ok(())
}

/*TODO: fix panic backtrace for Coverage CI
#[test]
fn pipeline_test_dynamic_construction_read_fail() -> Result<()> {
    let active = Rc::new(AtomicUsize::new(0));
    let inactive = Rc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline.add_back(MockHandler::<String, String>::new(
        "mock1",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    ))?;
    pipeline.add_back(MockHandler::<String, String>::new(
        "mock2",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    ))?;

    pipeline.add_front(MockHandler::<String, usize>::new(
        "mock3",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    ))?;

    let pipeline = pipeline.finalize()?;

    // Wrap it all in a std::sync::Mutex
    let pipeline = Arc::new(std::sync::Mutex::new(pipeline));
    let result = std::panic::catch_unwind(move || {
        let pipeline = pipeline.lock().unwrap();
        pipeline.read("TESTING INBOUND MESSAGE".to_owned());
    });
    assert!(result.is_err());

    Ok(())
}

#[test]
fn pipeline_test_dynamic_construction_write_fail() -> Result<()> {
    let active = Rc::new(AtomicUsize::new(0));
    let inactive = Rc::new(AtomicUsize::new(0));

    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline.add_back(MockHandler::<String, String>::new(
        "mock1",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    ))?;
    pipeline.add_back(MockHandler::<String, String>::new(
        "mock2",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    ))?;

    pipeline.add_front(MockHandler::<String, usize>::new(
        "mock3",
        Stats {
            active: Some(active.clone()),
            inactive: Some(inactive.clone()),
            ..Default::default()
        },
    ))?;

    let pipeline = pipeline.finalize()?;

    // Wrap it all in a std::sync::Mutex
    let pipeline = Arc::new(std::sync::Mutex::new(pipeline));
    let result = std::panic::catch_unwind(|| {
        let pipeline = pipeline.lock().unwrap();
        pipeline.write("TESTING OUTBOUND MESSAGE".to_owned());
    });
    assert!(result.is_err());

    Ok(())
}
*/

#[test]
fn pipeline_test_remove_handler() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))?;

    // Exercise both addFront and addBack. Final pipeline is
    // StI <-> ItS <-> StS <-> StS <-> StI <-> ItS
    pipeline
        .add_front(MockHandler::<usize, String>::new(
            "handler3",
            Stats::default(),
        ))?
        .add_front(MockHandler::<String, usize>::new(
            "handler4",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, usize>::new(
            "handler5",
            Stats::default(),
        ))?
        .add_back(MockHandler::<usize, String>::new(
            "handler6",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;

    pipeline.remove("handler3")?;
    pipeline.remove("handler4")?;
    pipeline.remove("handler5")?;
    pipeline.remove("handler6")?;
    let pipeline = pipeline.update()?;

    pipeline.read("TESTING INBOUND MESSAGE".to_owned());
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned());

    assert_eq!(2, pipeline.len());

    Ok(())
}

#[test]
fn pipeline_test_remove_front_back() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, String>::new(
            "handler2",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, String>::new(
            "handler3",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;

    pipeline.remove_front()?.remove_back()?;
    let pipeline = pipeline.update()?;

    assert_eq!(1, pipeline.len());

    pipeline.read("TESTING INBOUND MESSAGE".to_owned());
    pipeline.write("TESTING OUTBOUND MESSAGE".to_owned());

    pipeline.remove("handler2")?;

    assert_eq!(0, pipeline.len());

    Ok(())
}

#[test]
fn pipeline_test_num_handlers() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline.add_back(MockHandler::<String, String>::new(
        "handler1",
        Stats::default(),
    ))?;
    assert_eq!(1, pipeline.len());

    pipeline.add_back(MockHandler::<String, String>::new(
        "handler2",
        Stats::default(),
    ))?;
    assert_eq!(2, pipeline.len());

    let pipeline = pipeline.finalize()?;
    assert_eq!(2, pipeline.len());

    pipeline.remove("handler1")?;
    assert_eq!(1, pipeline.len());

    pipeline.remove("handler2")?;
    assert_eq!(0, pipeline.len());

    Ok(())
}

#[test]
fn pipeline_test_get_inbound_handler() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, usize>::new(
            "handler2",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;
    assert_eq!(2, pipeline.len());

    let handler = pipeline.get_inbound_handler("handler1");
    if let Some(handler) = handler {
        let handler = handler.borrow();
        let handler_any = handler.as_any_internal();
        let inbound_handler =
            handler_any.downcast_ref::<Box<dyn InboundHandler<Rin = String, Rout = String>>>();
        if let Some(inbound_handler) = inbound_handler {
            let inbound_handler_any = inbound_handler.as_any();
            let mock_decoder = inbound_handler_any.downcast_ref::<MockDecoder<String, String>>();
            assert!(mock_decoder.is_some());
            assert_eq!(mock_decoder.unwrap().name, "handler1 decoder");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn pipeline_test_get_outbound_handler() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, usize>::new(
            "handler2",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;
    assert_eq!(2, pipeline.len());

    let handler = pipeline.get_outbound_handler("handler2");
    if let Some(handler) = handler {
        let handler = handler.borrow();
        let handler_any = handler.as_any_internal();
        let outbound_handler =
            handler_any.downcast_ref::<Box<dyn OutboundHandler<Win = usize, Wout = String>>>();
        if let Some(outbound_handler) = outbound_handler {
            let outbound_handler_any = outbound_handler.as_any();
            let mock_encoder = outbound_handler_any.downcast_ref::<MockEncoder<usize, String>>();
            assert!(mock_encoder.is_some());
            assert_eq!(mock_encoder.unwrap().name, "handler2 encoder");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn pipeline_test_get_inbound_context() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, usize>::new(
            "handler2",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;
    assert_eq!(2, pipeline.len());

    let context = pipeline.get_inbound_context("handler1");
    if let Some(context) = context {
        let context = context.borrow();
        let context_any = context.as_any_internal();
        let inbound_context = context_any.downcast_ref::<InboundContext<String, String>>();
        if let Some(inbound_context) = inbound_context {
            assert_eq!(inbound_context.name(), "handler1");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }

    Ok(())
}

#[test]
fn pipeline_test_get_outbound_context() -> Result<()> {
    let pipeline: Pipeline<String, String> = Pipeline::new();
    pipeline
        .add_back(MockHandler::<String, String>::new(
            "handler1",
            Stats::default(),
        ))?
        .add_back(MockHandler::<String, usize>::new(
            "handler2",
            Stats::default(),
        ))?;
    let pipeline = pipeline.finalize()?;
    assert_eq!(2, pipeline.len());

    let context = pipeline.get_outbound_context("handler2");
    if let Some(context) = context {
        let context = context.borrow();
        let context_any = context.as_any_internal();
        let outbound_context = context_any.downcast_ref::<OutboundContext<usize, String>>();
        if let Some(outbound_context) = outbound_context {
            assert_eq!(outbound_context.name(), "handler2");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }

    Ok(())
}
