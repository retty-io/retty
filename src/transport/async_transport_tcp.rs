use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::transport::AsyncTransportWrite;

struct AsyncTransportTcpDecoder;
struct AsyncTransportTcpEncoder {
    writer: Option<Box<dyn AsyncTransportWrite + Send + Sync>>,
}

/// Asynchronous TCP transport handler that reads with input of BytesMut and output of BytesMut,
/// or writes with input of BytesMut and output of BytesMut
pub struct AsyncTransportTcp {
    decoder: AsyncTransportTcpDecoder,
    encoder: AsyncTransportTcpEncoder,
}

impl AsyncTransportTcp {
    /// Creates a new asynchronous transport handler for TCP
    pub fn new(writer: Box<dyn AsyncTransportWrite + Send + Sync>) -> Self {
        AsyncTransportTcp {
            decoder: AsyncTransportTcpDecoder {},
            encoder: AsyncTransportTcpEncoder {
                writer: Some(writer),
            },
        }
    }
}

#[async_trait]
impl InboundHandler for AsyncTransportTcpDecoder {
    type Rin = BytesMut;
    type Rout = Self::Rin;

    async fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        ctx.fire_read(msg).await;
    }
}

#[async_trait]
impl OutboundHandler for AsyncTransportTcpEncoder {
    type Win = BytesMut;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        if let Some(writer) = &mut self.writer {
            match writer.write(&msg, None).await {
                Ok(n) => {
                    trace!(
                        "AsyncTransportTcpEncoder --> write {} of {} bytes",
                        n,
                        msg.len()
                    );
                }
                Err(err) => {
                    warn!("AsyncTransportTcpEncoder write error: {}", err);
                    ctx.fire_write_exception(err.into()).await;
                }
            };
        }
    }
    async fn close(&mut self, _ctx: &OutboundContext<Self::Win, Self::Wout>) {
        trace!("close socket");
        self.writer.take();
    }
}

impl Handler for AsyncTransportTcp {
    type Rin = BytesMut;
    type Rout = Self::Rin;
    type Win = BytesMut;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "AsyncTransportTcp"
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
