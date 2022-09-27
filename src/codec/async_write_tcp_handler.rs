use crate::channel::handler::{Handler, InboundHandler, OutboundHandler, OutboundHandlerContext};

use async_trait::async_trait;
use bytes::BytesMut;
use log::trace;
use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;

struct AsyncTcpDecoder;
struct AsyncTcpEncoder {
    writer: Option<Pin<Box<dyn AsyncWrite + Send + Sync>>>,
}

pub struct AsyncWriteTcpHandler {
    decoder: AsyncTcpDecoder,
    encoder: AsyncTcpEncoder,
}

impl AsyncWriteTcpHandler {
    pub fn new(writer: Pin<Box<dyn AsyncWrite + Send + Sync>>) -> Self {
        AsyncWriteTcpHandler {
            decoder: AsyncTcpDecoder {},
            encoder: AsyncTcpEncoder {
                writer: Some(writer),
            },
        }
    }
}

impl InboundHandler for AsyncTcpDecoder {}

#[async_trait]
impl OutboundHandler for AsyncTcpEncoder {
    async fn write(
        &mut self,
        _ctx: &mut OutboundHandlerContext,
        message: &mut (dyn Any + Send + Sync),
    ) {
        let buf = message.downcast_mut::<BytesMut>().unwrap();
        if let Some(writer) = &mut self.writer {
            if let Ok(n) = writer.write(buf).await {
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

impl Handler for AsyncWriteTcpHandler {
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
