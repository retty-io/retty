//! Handlers for converting byte to message

use async_trait::async_trait;
use bytes::BytesMut;
use std::sync::Arc;

use crate::channel::{
    Handler, InboundHandler, InboundHandlerContext, InboundHandlerInternal, OutboundHandler,
    OutboundHandlerContext, OutboundHandlerInternal,
};
use crate::runtime::sync::Mutex;

mod line_based_frame_decoder;
mod tagged;

pub use line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType};
pub use tagged::TaggedByteToMessageCodec;

/// This trait allows for decoding messages.
pub trait MessageDecoder {
    /// Decodes byte buffer to message buffer
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, std::io::Error>;
}

struct ByteToMessageDecoder {
    transport_active: bool,
    message_decoder: Box<dyn MessageDecoder + Send + Sync>,
}

struct ByteToMessageEncoder;

/// A Byte to Message Codec handler that reads with input of BytesMut and output of BytesMut,
/// or writes with input of BytesMut and output of BytesMut
pub struct ByteToMessageCodec {
    decoder: ByteToMessageDecoder,
    encoder: ByteToMessageEncoder,
}

impl ByteToMessageCodec {
    /// Creates a new ByteToMessageCodec handler
    pub fn new(message_decoder: Box<dyn MessageDecoder + Send + Sync>) -> Self {
        Self {
            decoder: ByteToMessageDecoder {
                transport_active: false,
                message_decoder,
            },
            encoder: ByteToMessageEncoder {},
        }
    }
}

#[async_trait]
impl InboundHandler for ByteToMessageDecoder {
    type Rin = BytesMut;
    type Rout = Self::Rin;

    async fn transport_active(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        self.transport_active = true;
        ctx.fire_transport_active().await;
    }
    async fn transport_inactive(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        self.transport_active = false;
        ctx.fire_transport_inactive().await;
    }
    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        message: &mut Self::Rin,
    ) {
        while self.transport_active {
            match self.message_decoder.decode(message) {
                Ok(msg) => {
                    if let Some(mut msg) = msg {
                        ctx.fire_read(&mut msg).await;
                    } else {
                        return;
                    }
                }
                Err(err) => {
                    ctx.fire_read_exception(Box::new(err)).await;
                    return;
                }
            }
        }
    }
}

#[async_trait]
impl OutboundHandler for ByteToMessageEncoder {
    type Win = BytesMut;
    type Wout = Self::Win;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        message: &mut Self::Win,
    ) {
        ctx.fire_write(message).await;
    }
}

impl Handler for ByteToMessageCodec {
    type Rin = BytesMut;
    type Rout = Self::Rin;
    type Win = BytesMut;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "ByteToMessageCodec"
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let inbound_handler: Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>> =
            Box::new(self.decoder);
        let outbound_handler: Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>> =
            Box::new(self.encoder);

        (
            Arc::new(Mutex::new(inbound_handler)),
            Arc::new(Mutex::new(outbound_handler)),
        )
    }
}
