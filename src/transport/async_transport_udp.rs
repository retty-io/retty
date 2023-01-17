use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::io::ErrorKind;
use std::sync::Arc;

use crate::channel::handler::{
    Handler, InboundHandler, InboundHandlerInternal, OutboundHandler, OutboundHandlerContext,
    OutboundHandlerInternal,
};
use crate::error::Error;
use crate::runtime::sync::Mutex;
use crate::transport::{AsyncTransportWrite, TransportContext};

struct AsyncTransportUdpDecoder;
struct AsyncTransportUdpEncoder {
    writer: Box<dyn AsyncTransportWrite + Send + Sync>,
}

pub struct AsyncTransportUdp {
    decoder: AsyncTransportUdpDecoder,
    encoder: AsyncTransportUdpEncoder,
}

impl AsyncTransportUdp {
    pub fn new(writer: Box<dyn AsyncTransportWrite + Send + Sync>) -> Self {
        AsyncTransportUdp {
            decoder: AsyncTransportUdpDecoder {},
            encoder: AsyncTransportUdpEncoder { writer },
        }
    }
}

pub struct TaggedBytesMut {
    pub transport: TransportContext,
    pub message: BytesMut,
}

impl InboundHandler<TaggedBytesMut> for AsyncTransportUdpDecoder {}

#[async_trait]
impl OutboundHandler<TaggedBytesMut> for AsyncTransportUdpEncoder {
    async fn write(&mut self, ctx: &mut OutboundHandlerContext, msg: &mut TaggedBytesMut) {
        if let Some(target) = msg.transport.peer_addr {
            match self.writer.write(&msg.message, Some(target)).await {
                Ok(n) => {
                    trace!(
                        "AsyncTransportUdpEncoder --> write {} of {} bytes",
                        n,
                        msg.message.len()
                    );
                }
                Err(err) => {
                    warn!("AsyncTransportUdpEncoder write error: {}", err);
                    ctx.fire_write_exception(err.into()).await;
                }
            }
        } else {
            let err = Error::new(
                ErrorKind::NotConnected,
                String::from("Transport endpoint is not connected"),
            );
            ctx.fire_write_exception(err).await;
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
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let decoder: Box<dyn InboundHandler<TaggedBytesMut>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandler<TaggedBytesMut>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
