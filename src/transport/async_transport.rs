use log::{trace, warn};
use std::any::Any;
use std::marker::PhantomData;

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::transport::AsyncTransportWrite;

struct AsyncTransportDecoder<R> {
    phantom: PhantomData<R>,
}
struct AsyncTransportEncoder<R> {
    writer: Option<AsyncTransportWrite<R>>,
}

/// Asynchronous transport handler that writes R
pub struct AsyncTransport<R> {
    decoder: AsyncTransportDecoder<R>,
    encoder: AsyncTransportEncoder<R>,
}

impl<R> AsyncTransport<R> {
    /// Creates a new asynchronous transport handler
    pub fn new(writer: AsyncTransportWrite<R>) -> Self {
        AsyncTransport {
            decoder: AsyncTransportDecoder {
                phantom: PhantomData,
            },
            encoder: AsyncTransportEncoder {
                writer: Some(writer),
            },
        }
    }
}

impl<R: 'static> InboundHandler for AsyncTransportDecoder<R> {
    type Rin = R;
    type Rout = Self::Rin;

    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        ctx.fire_read(msg);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<R: 'static> OutboundHandler for AsyncTransportEncoder<R> {
    type Win = R;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        if let Some(writer) = &self.writer {
            if let Err(err) = writer.write(msg) {
                warn!("AsyncTransport write error: {:?}", err);
                ctx.fire_write_exception(Box::new(err));
            };
        }
    }
    fn close(&mut self, _ctx: &OutboundContext<Self::Win, Self::Wout>) {
        trace!("close AsyncTransport");
        self.writer.take();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<R: 'static> Handler for AsyncTransport<R> {
    type Rin = R;
    type Rout = Self::Rin;
    type Win = Self::Rin;
    type Wout = Self::Rin;

    fn name(&self) -> &str {
        "AsyncTransport"
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
