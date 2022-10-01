use crate::channel::handler::{Handler, InboundHandler, OutboundHandler, OutboundHandlerContext};
use crate::transport::AsyncTransportWrite;

use async_trait::async_trait;
use bytes::BytesMut;
use log::trace;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    async fn write(
        &mut self,
        _ctx: &mut OutboundHandlerContext,
        message: &mut (dyn Any + Send + Sync),
    ) {
        let buf = message.downcast_mut::<BytesMut>().unwrap();
        if let Some(writer) = &mut self.writer {
            //TODO: add TransportContext
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

impl Handler for AsyncTransportUdp {
    fn id(&self) -> String {
        "AsyncWriteUdpHandler".to_string()
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
