use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;

use crate::channel::handler::{
    Handler, InboundHandler, InboundHandlerInternal, OutboundHandler, OutboundHandlerContext,
    OutboundHandlerInternal,
};
use crate::runtime::sync::Mutex;
use crate::transport::AsyncTransportWrite;

struct AsyncTransportTcpDecoder;
struct AsyncTransportTcpEncoder {
    writer: Option<Box<dyn AsyncTransportWrite + Send + Sync>>,
}

pub struct AsyncTransportTcp {
    decoder: AsyncTransportTcpDecoder,
    encoder: AsyncTransportTcpEncoder,
}

impl AsyncTransportTcp {
    pub fn new(writer: Box<dyn AsyncTransportWrite + Send + Sync>) -> Self {
        AsyncTransportTcp {
            decoder: AsyncTransportTcpDecoder {},
            encoder: AsyncTransportTcpEncoder {
                writer: Some(writer),
            },
        }
    }
}

impl InboundHandler<BytesMut> for AsyncTransportTcpDecoder {}

#[async_trait]
impl OutboundHandler<BytesMut> for AsyncTransportTcpEncoder {
    async fn write(&mut self, ctx: &mut OutboundHandlerContext, message: &mut BytesMut) {
        if let Some(writer) = &mut self.writer {
            match writer.write(message, None).await {
                Ok(n) => {
                    trace!(
                        "AsyncTransportTcpEncoder --> write {} of {} bytes",
                        n,
                        message.len()
                    );
                }
                Err(err) => {
                    warn!("AsyncTransportTcpEncoder write error: {}", err);
                    ctx.fire_write_exception(err.into()).await;
                }
            };
        }
    }
    async fn close(&mut self, _ctx: &mut OutboundHandlerContext) {
        trace!("close socket");
        self.writer.take();
    }
}

impl Handler for AsyncTransportTcp {
    fn id(&self) -> String {
        "AsyncTransportTcp".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let decoder: Box<dyn InboundHandler<BytesMut>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandler<BytesMut>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
