use async_trait::async_trait;

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::codec::byte_to_message_decoder::MessageDecoder;
use crate::transport::TaggedBytesMut;

struct TaggedByteToMessageDecoder {
    transport_active: bool,
    message_decoder: Box<dyn MessageDecoder + Send + Sync>,
}

struct TaggedByteToMessageEncoder;

/// A tagged Byte to Message Codec handler that reads with input of TaggedBytesMut and output of TaggedBytesMut,
/// or writes with input of TaggedBytesMut and output of TaggedBytesMut
pub struct TaggedByteToMessageCodec {
    decoder: TaggedByteToMessageDecoder,
    encoder: TaggedByteToMessageEncoder,
}

impl TaggedByteToMessageCodec {
    /// Creates a new TaggedByteToMessageCodec handler
    pub fn new(message_decoder: Box<dyn MessageDecoder + Send + Sync>) -> Self {
        Self {
            decoder: TaggedByteToMessageDecoder {
                transport_active: false,
                message_decoder,
            },
            encoder: TaggedByteToMessageEncoder {},
        }
    }
}

#[async_trait]
impl InboundHandler for TaggedByteToMessageDecoder {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;

    async fn transport_active(&mut self, ctx: &mut InboundContext<Self::Rin, Self::Rout>) {
        self.transport_active = true;
        ctx.fire_transport_active().await;
    }
    async fn transport_inactive(&mut self, ctx: &mut InboundContext<Self::Rin, Self::Rout>) {
        self.transport_active = false;
        ctx.fire_transport_inactive().await;
    }
    async fn read(&mut self, ctx: &mut InboundContext<Self::Rin, Self::Rout>, mut msg: Self::Rin) {
        while self.transport_active {
            match self.message_decoder.decode(&mut msg.message) {
                Ok(message) => {
                    if let Some(message) = message {
                        ctx.fire_read(TaggedBytesMut {
                            transport: msg.transport,
                            message,
                        })
                        .await;
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
impl OutboundHandler for TaggedByteToMessageEncoder {
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &mut OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg).await;
    }
}

impl Handler for TaggedByteToMessageCodec {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "TaggedByteToMessageCodec"
    }

    fn split(
        self,
    ) -> (
        Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
        Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
    ) {
        (Box::new(self.decoder), Box::new(self.encoder))
    }
}
