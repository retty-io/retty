use crate::channel::handler::*;
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex;

struct StringDecoder;
struct StringEncoder;

pub struct StringCodec {
    decoder: StringDecoder,
    encoder: StringEncoder,
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
    async fn transport_active(&mut self, ctx: &mut InboundHandlerContext) {
        ctx.fire_transport_active().await;
    }

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext,
        message: &mut (dyn Any + Send + Sync),
    ) {
        let buf = message.downcast_mut::<BytesMut>().unwrap();

        match String::from_utf8(buf.to_vec()) {
            Ok(mut msg) => {
                ctx.fire_read(&mut msg).await;
            }
            Err(err) => ctx.fire_read_exception(err.into()).await,
        }
    }
}

#[async_trait]
impl OutboundHandler for StringEncoder {
    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext,
        message: &mut (dyn Any + Send + Sync),
    ) {
        let msg = message.downcast_ref::<String>().unwrap();
        let mut buf = BytesMut::new();
        buf.put(msg.as_bytes());
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
        let (decoder, encoder) = (self.decoder, self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}
