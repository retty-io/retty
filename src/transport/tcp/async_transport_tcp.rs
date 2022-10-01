use crate::channel::handler::{Handler, InboundHandler, OutboundHandler, OutboundHandlerContext};
use crate::transport::AsyncTransportWrite;

use async_trait::async_trait;
use bytes::BytesMut;
use log::trace;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    async fn write(
        &mut self,
        _ctx: &mut OutboundHandlerContext,
        message: &mut (dyn Any + Send + Sync),
    ) {
        let buf = message.downcast_mut::<BytesMut>().unwrap();
        if let Some(writer) = &mut self.writer {
            if let Ok(n) = writer.write(buf, None).await {
                trace!(
                    "AsyncWriteTcpHandler --> write {} of {} bytes",
                    n,
                    buf.len()
                );
            }
        }
    }

    async fn close(&mut self, _ctx: &mut OutboundHandlerContext) {
        trace!("close socket");
        self.writer.take();
    }
}

impl Handler for AsyncTransportTcp {
    fn id(&self) -> String {
        "AsyncWriteTcpHandler".to_string()
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
