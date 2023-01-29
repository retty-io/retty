use super::*;
use crate::channel::{
    Handler, InboundHandler, InboundHandlerContext, OutboundHandler, OutboundHandlerContext,
};
use crate::runtime::mpsc::Sender;
use bytes::BytesMut;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

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

struct MockDecoder<Rin, Rout> {
    name: String,
    active: Arc<AtomicUsize>,
    inactive: Arc<AtomicUsize>,

    phantom_in: PhantomData<Rin>,
    phantom_out: PhantomData<Rout>,
}
struct MockEncoder<Win, Wout> {
    phantom_in: PhantomData<Win>,
    phantom_out: PhantomData<Wout>,
}
pub(crate) struct MockHandler<R, W> {
    decoder: MockDecoder<R, W>,
    encoder: MockEncoder<W, R>,
}

impl<R, W> MockHandler<R, W> {
    pub fn new(name: &str, active: Arc<AtomicUsize>, inactive: Arc<AtomicUsize>) -> Self {
        MockHandler {
            decoder: MockDecoder {
                name: name.to_string(),
                active,
                inactive,

                phantom_in: PhantomData,
                phantom_out: PhantomData,
            },
            encoder: MockEncoder {
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

    async fn transport_active(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        self.active.fetch_add(1, Ordering::SeqCst);
        ctx.fire_transport_active().await;
    }

    async fn transport_inactive(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        self.inactive.fetch_add(1, Ordering::SeqCst);
        ctx.fire_transport_inactive().await;
    }

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        _msg: Self::Rin,
    ) {
        ctx.fire_read(Rout::default()).await;
    }
}

#[async_trait]
impl<Win: Default + Send + Sync + 'static, Wout: Default + Send + Sync + 'static> OutboundHandler
    for MockEncoder<Win, Wout>
{
    type Win = Win;
    type Wout = Wout;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        _msg: Self::Win,
    ) {
        ctx.fire_write(Wout::default()).await;
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
