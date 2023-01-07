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

impl InboundHandler for StringDecoder {}

#[async_trait]
impl InboundHandlerAdapter<BytesMut> for StringDecoder {
    async fn read_type(&mut self, ctx: &mut InboundHandlerContext, message: &mut BytesMut) {
        match String::from_utf8(message.to_vec()) {
            Ok(mut msg) => {
                ctx.fire_read(&mut msg).await;
            }
            Err(err) => ctx.fire_read_exception(err.into()).await,
        }
    }
}

impl OutboundHandler for StringEncoder {}

#[async_trait]
impl OutboundHandlerAdapter<String> for StringEncoder {
    async fn write_type(&mut self, ctx: &mut OutboundHandlerContext, message: &mut String) {
        let mut buf = BytesMut::new();
        buf.put(message.as_bytes());
        let mut bytes = buf.freeze();
        ctx.fire_write(&mut bytes).await;
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
        let (decoder, encoder) = (self.decoder, self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
