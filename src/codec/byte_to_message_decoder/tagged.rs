use async_trait::async_trait;
use std::sync::Arc;

use crate::channel::handler::*;
use crate::channel::handler_internal::{InboundHandlerInternal, OutboundHandlerInternal};
use crate::codec::byte_to_message_decoder::MessageDecoder;
use crate::runtime::sync::Mutex;
use crate::transport::async_transport_udp::TaggedBytesMut;

struct TaggedByteToMessageDecoder {
    transport_active: bool,
    message_decoder: Box<dyn MessageDecoder + Send + Sync>,
}

struct TaggedByteToMessageEncoder;

pub struct TaggedByteToMessageCodec {
    decoder: TaggedByteToMessageDecoder,
    encoder: TaggedByteToMessageEncoder,
}

impl TaggedByteToMessageCodec {
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
        msg: &mut Self::Rin,
    ) {
        while self.transport_active {
            match self.message_decoder.decode(&mut msg.message) {
                Ok(message) => {
                    if let Some(message) = message {
                        ctx.fire_read(&mut TaggedBytesMut {
                            transport: msg.transport,
                            message,
                        })
                        .await;
                    } else {
                        return;
                    }
                }
                Err(err) => {
                    ctx.fire_read_exception(err).await;
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

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        message: &mut Self::Win,
    ) {
        ctx.fire_write(message).await;
    }
}

impl Handler for TaggedByteToMessageCodec {
    type In = TaggedBytesMut;
    type Out = Self::In;

    fn name(&self) -> &str {
        "TaggedByteToMessageCodec"
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let inbound_handler: Box<dyn InboundHandler<Rin = Self::In, Rout = Self::Out>> =
            Box::new(self.decoder);
        let outbound_handler: Box<dyn OutboundHandler<Win = Self::Out, Wout = Self::In>> =
            Box::new(self.encoder);

        (
            Arc::new(Mutex::new(inbound_handler)),
            Arc::new(Mutex::new(outbound_handler)),
        )
    }
}
