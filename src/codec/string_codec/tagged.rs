use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;

use crate::channel::handler::*;
use crate::codec::string_codec::{StringDecoder, StringEncoder};
use crate::runtime::sync::Mutex;
use crate::transport::async_transport_udp::TaggedBytesMut;
use crate::transport::TransportContext;

pub struct TaggedStringCodec {
    decoder: StringDecoder,
    encoder: StringEncoder,
}

impl Default for TaggedStringCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl TaggedStringCodec {
    pub fn new() -> Self {
        TaggedStringCodec {
            decoder: StringDecoder {},
            encoder: StringEncoder {},
        }
    }
}

pub struct TaggedString {
    pub transport: TransportContext,
    pub message: String,
}

#[async_trait]
impl InboundHandlerGeneric<TaggedBytesMut> for StringDecoder {
    async fn read_generic(&mut self, ctx: &mut InboundHandlerContext, msg: &mut TaggedBytesMut) {
        match String::from_utf8(msg.message.to_vec()) {
            Ok(message) => {
                ctx.fire_read(&mut TaggedString {
                    transport: msg.transport,
                    message,
                })
                .await;
            }
            Err(err) => ctx.fire_read_exception(err.into()).await,
        }
    }
}

#[async_trait]
impl OutboundHandlerGeneric<TaggedString> for StringEncoder {
    async fn write_generic(&mut self, ctx: &mut OutboundHandlerContext, msg: &mut TaggedString) {
        let mut buf = BytesMut::new();
        buf.put(msg.message.as_bytes());
        ctx.fire_write(&mut TaggedBytesMut {
            transport: msg.transport,
            message: buf,
        })
        .await;
    }
}

impl Handler for TaggedStringCodec {
    fn id(&self) -> String {
        "TaggedStringCodec Handler".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let decoder: Box<dyn InboundHandlerGeneric<TaggedBytesMut>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandlerGeneric<TaggedString>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
