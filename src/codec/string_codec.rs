use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;

use crate::channel::handler::*;
use crate::runtime::sync::Mutex;

struct StringDecoder;
struct StringEncoder;

pub struct StringCodec {
    decoder: StringDecoder,
    encoder: StringEncoder,
}

impl Default for StringCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl StringCodec {
    pub fn new() -> Self {
        StringCodec {
            decoder: StringDecoder {},
            encoder: StringEncoder {},
        }
    }
}

#[async_trait]
impl InboundHandlerGeneric<BytesMut> for StringDecoder {
    async fn read_generic(&mut self, ctx: &mut InboundHandlerContext, message: &mut BytesMut) {
        match String::from_utf8(message.to_vec()) {
            Ok(mut msg) => {
                ctx.fire_read(&mut msg).await;
            }
            Err(err) => ctx.fire_read_exception(err.into()).await,
        }
    }
}

#[async_trait]
impl OutboundHandlerGeneric<String> for StringEncoder {
    async fn write_generic(&mut self, ctx: &mut OutboundHandlerContext, message: &mut String) {
        let mut buf = BytesMut::new();
        buf.put(message.as_bytes());
        ctx.fire_write(&mut buf).await;
    }
}

impl Handler for StringCodec {
    fn id(&self) -> String {
        "StringCodec Handler".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandler>>,
        Arc<Mutex<dyn OutboundHandler>>,
    ) {
        let decoder: Box<dyn InboundHandlerGeneric<BytesMut>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandlerGeneric<String>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
