use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;

use crate::channel::handler::{
    Handler, InboundHandler, InboundHandlerGeneric, OutboundHandler, OutboundHandlerContext,
    OutboundHandlerGeneric,
};
use crate::runtime::sync::Mutex;
use crate::transport::AsyncTransportWrite;

struct AsyncTransportUdpDecoder;
struct AsyncTransportUdpEncoder {
    writer: Option<Box<dyn AsyncTransportWrite + Send + Sync>>,
}

pub struct AsyncTransportUdp {
    decoder: AsyncTransportUdpDecoder,
    encoder: AsyncTransportUdpEncoder,
}

impl AsyncTransportUdp {
    pub fn new(writer: Box<dyn AsyncTransportWrite + Send + Sync>) -> Self {
        AsyncTransportUdp {
            decoder: AsyncTransportUdpDecoder {},
            encoder: AsyncTransportUdpEncoder {
                writer: Some(writer),
            },
        }
    }
}

impl InboundHandlerGeneric<BytesMut> for AsyncTransportUdpDecoder {}

#[async_trait]
impl OutboundHandlerGeneric<BytesMut> for AsyncTransportUdpEncoder {
    async fn write_generic(&mut self, ctx: &mut OutboundHandlerContext, message: &mut BytesMut) {
        if let Some(writer) = &mut self.writer {
            //TODO: if let Some(target) = message.transport.peer_addr {
            match writer.write(message, None /*Some(target)*/).await {
                Ok(n) => {
                    trace!(
                        "AsyncTransportUdpEncoder --> write {} of {} bytes",
                        n,
                        message.len()
                    );
                }
                Err(err) => {
                    warn!("AsyncTransportUdpEncoder write error: {}", err);
                    ctx.fire_write_exception(err.into()).await;
                }
            };
            /*} else {
                let err = Error::new(
                    ErrorKind::NotConnected,
                    String::from("Transport endpoint is not connected"),
                );
                ctx.fire_write_exception(err).await;
            }*/
        }
    }
    async fn close_generic(&mut self, _ctx: &mut OutboundHandlerContext) {
        trace!("close socket");
        self.writer.take();
    }
}

impl Handler for AsyncTransportUdp {
    fn id(&self) -> String {
        "AsyncTransportUdp".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandler>>,
        Arc<Mutex<dyn OutboundHandler>>,
    ) {
        let decoder: Box<dyn InboundHandlerGeneric<BytesMut>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandlerGeneric<BytesMut>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
