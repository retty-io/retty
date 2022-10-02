use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::channel::handler::*;
use crate::error::Error;
use crate::Message;

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
impl InboundHandler for StringDecoder {
    async fn read(&mut self, ctx: &mut InboundHandlerContext, mut message: Message) {
        if let Some(buf) = message.body.downcast_mut::<BytesMut>() {
            match String::from_utf8(buf.to_vec()) {
                Ok(msg) => {
                    message.body = Box::new(msg);
                    ctx.fire_read(message).await;
                }
                Err(err) => ctx.fire_read_exception(err.into()).await,
            }
        } else {
            let err = Error::new(
                ErrorKind::Other,
                String::from("message.body.downcast_mut::<BytesMut> error"),
            );
            ctx.fire_read_exception(err).await;
        }
    }
}

#[async_trait]
impl OutboundHandler for StringEncoder {
    async fn write(&mut self, ctx: &mut OutboundHandlerContext, mut message: Message) {
        if let Some(msg) = message.body.downcast_mut::<String>() {
            let mut buf = BytesMut::new();
            buf.put(msg.as_bytes());
            message.body = Box::new(buf);
            ctx.fire_write(message).await;
        } else {
            let err = Error::new(
                ErrorKind::Other,
                String::from("message.body.downcast_mut::<String> error"),
            );
            ctx.fire_write_exception(err).await;
        }
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
