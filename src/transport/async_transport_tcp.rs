use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::channel::handler::{Handler, InboundHandler, OutboundHandler, OutboundHandlerContext};
use crate::error::Error;
use crate::transport::AsyncTransportWrite;
use crate::Message;

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

impl InboundHandler for AsyncTransportTcpDecoder {}

#[async_trait]
impl OutboundHandler for AsyncTransportTcpEncoder {
    async fn write(&mut self, ctx: &mut OutboundHandlerContext, mut message: Message) {
        if let Some(buf) = message.body.downcast_mut::<BytesMut>() {
            if let Some(writer) = &mut self.writer {
                match writer.write(buf, None).await {
                    Ok(n) => {
                        trace!(
                            "AsyncTransportTcpEncoder --> write {} of {} bytes",
                            n,
                            buf.len()
                        );
                    }
                    Err(err) => {
                        warn!("AsyncTransportTcpEncoder write error: {}", err);
                        ctx.fire_write_exception(err.into()).await;
                    }
                };
            }
        } else {
            let err = Error::new(
                ErrorKind::Other,
                String::from("message.body.downcast_mut::<BytesMut> error"),
            );
            ctx.fire_write_exception(err).await;
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
        Arc<Mutex<dyn InboundHandler>>,
        Arc<Mutex<dyn OutboundHandler>>,
    ) {
        let (decoder, encoder) = (self.decoder, self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
