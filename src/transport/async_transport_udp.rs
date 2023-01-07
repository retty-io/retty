use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;

use crate::channel::handler::{
    Handler, InboundHandler, OutboundHandler, OutboundHandlerAdapter, OutboundHandlerContext,
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

impl InboundHandler for AsyncTransportUdpDecoder {}

#[async_trait]
impl OutboundHandler for AsyncTransportUdpEncoder {
    async fn close(&mut self, _ctx: &mut OutboundHandlerContext) {
        trace!("close socket");
        self.writer.take();
    }
}

#[async_trait]
impl OutboundHandlerAdapter<BytesMut> for AsyncTransportUdpEncoder {
    async fn write_type(&mut self, ctx: &mut OutboundHandlerContext, message: &mut BytesMut) {
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
        let (decoder, encoder) = (self.decoder, self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
