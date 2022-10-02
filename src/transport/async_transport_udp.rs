use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::io::ErrorKind;
use std::sync::Arc;

use crate::channel::handler::{Handler, InboundHandler, OutboundHandler, OutboundHandlerContext};
use crate::error::Error;
use crate::runtime::sync::Mutex;
use crate::transport::AsyncTransportWrite;
use crate::Message;

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
    async fn write(&mut self, ctx: &mut OutboundHandlerContext, mut message: Message) {
        if let Some(writer) = &mut self.writer {
            let target = message.transport.peer_addr;
            if let Some(buf) = message.body.downcast_mut::<BytesMut>() {
                match writer.write(buf, Some(target)).await {
                    Ok(n) => {
                        trace!(
                            "AsyncTransportUdpEncoder --> write {} of {} bytes",
                            n,
                            buf.len()
                        );
                    }
                    Err(err) => {
                        warn!("AsyncTransportUdpEncoder write error: {}", err);
                        ctx.fire_write_exception(err.into()).await;
                    }
                };
            } else {
                let err = Error::new(
                    ErrorKind::Other,
                    String::from("message.body.downcast_mut::<BytesMut> error"),
                );
                ctx.fire_write_exception(err).await;
            }
        }
    }

    async fn close(&mut self, _ctx: &mut OutboundHandlerContext) {
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
        let (decoder, encoder) = (self.decoder, self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
