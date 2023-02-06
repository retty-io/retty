use log::{trace, warn};
use std::io::ErrorKind;
use std::marker::PhantomData;
use tokio::sync::broadcast::Sender;

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};

struct AsyncTransportDecoder<T> {
    phantom: PhantomData<T>,
}
struct AsyncTransportEncoder<T> {
    writer: Option<Sender<T>>,
}

/// Asynchronous transport handler that reads T and writes T
pub struct AsyncTransport<T> {
    decoder: AsyncTransportDecoder<T>,
    encoder: AsyncTransportEncoder<T>,
}

impl<T> AsyncTransport<T> {
    /// Creates a new asynchronous transport handler
    pub fn new(writer: Sender<T>) -> Self {
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

impl<T: Send + 'static> InboundHandler for AsyncTransportDecoder<T> {
    type Rin = T;
    type Rout = Self::Rin;

    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        ctx.fire_read(msg);
    }
}

impl<T: Send + 'static> OutboundHandler for AsyncTransportEncoder<T> {
    type Win = T;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        if let Some(writer) = &self.writer {
            if let Err(err) = writer.send(msg) {
                warn!("AsyncTransport write error: {}", err);
                ctx.fire_write_exception(Box::new(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    format!("{}", err),
                )));
            };
        }
    }
    fn close(&mut self, _ctx: &OutboundContext<Self::Win, Self::Wout>) {
        trace!("close AsyncTransport");
        self.writer.take();
    }
}

impl<T: Send + 'static> Handler for AsyncTransport<T> {
    type Rin = T;
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
