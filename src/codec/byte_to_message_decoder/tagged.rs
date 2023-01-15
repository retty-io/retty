use async_trait::async_trait;
use std::sync::Arc;

use crate::channel::handler::*;
use crate::codec::byte_to_message_decoder::{
    ByteToMessageDecoder, ByteToMessageEncoder, MessageDecoder,
};
use crate::runtime::sync::Mutex;
use crate::transport::async_transport_udp::TaggedBytesMut;

pub struct TaggedByteToMessageCodec {
    decoder: ByteToMessageDecoder,
    encoder: ByteToMessageEncoder,
}

impl TaggedByteToMessageCodec {
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
impl InboundHandlerGeneric<TaggedBytesMut> for ByteToMessageDecoder {
    async fn transport_active_generic(&mut self, ctx: &mut InboundHandlerContext) {
        self.transport_active = true;
        ctx.fire_transport_active().await;
    }
    async fn transport_inactive_generic(&mut self, ctx: &mut InboundHandlerContext) {
        self.transport_active = false;
        ctx.fire_transport_inactive().await;
    }
    async fn read_generic(&mut self, ctx: &mut InboundHandlerContext, msg: &mut TaggedBytesMut) {
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

impl OutboundHandlerGeneric<TaggedBytesMut> for ByteToMessageEncoder {}

impl Handler for TaggedByteToMessageCodec {
    fn id(&self) -> String {
        self.decoder.message_decoder.id()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandler>>,
        Arc<Mutex<dyn OutboundHandler>>,
    ) {
        let decoder: Box<dyn InboundHandlerGeneric<TaggedBytesMut>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandlerGeneric<TaggedBytesMut>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
