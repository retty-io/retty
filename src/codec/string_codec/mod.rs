//pub mod tagged;

use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;

use crate::channel::handler::*;
use crate::channel::handler_internal::{
    InboundHandlerContextInternal, InboundHandlerInternal, OutboundHandlerContextInternal,
    OutboundHandlerInternal,
};
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
impl InboundHandler for StringDecoder {
    type Rin = BytesMut;
    type Rout = String;

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        msg: &mut Self::Rin,
    ) {
        match String::from_utf8(msg.to_vec()) {
            Ok(mut message) => {
                ctx.fire_read(&mut message).await;
            }
            Err(err) => ctx.fire_read_exception(err.into()).await,
        }
    }
}

#[async_trait]
impl OutboundHandler for StringEncoder {
    type Win = String;
    type Wout = BytesMut;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        message: &mut Self::Win,
    ) {
        let mut buf = BytesMut::new();
        buf.put(message.as_bytes());
        ctx.fire_write(&mut buf).await;
    }
}

impl Handler for StringCodec {
    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerContextInternal>>,
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerContextInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let inbound_context: InboundHandlerContext<BytesMut, String> =
            InboundHandlerContext::default();
        let inbound_handler: Box<dyn InboundHandler<Rin = BytesMut, Rout = String>> =
            Box::new(self.decoder);
        let outbound_context: OutboundHandlerContext<String, BytesMut> =
            OutboundHandlerContext::default();
        let outbound_handler: Box<dyn OutboundHandler<Win = String, Wout = BytesMut>> =
            Box::new(self.encoder);

        (
            Arc::new(Mutex::new(inbound_context)),
            Arc::new(Mutex::new(inbound_handler)),
            Arc::new(Mutex::new(outbound_context)),
            Arc::new(Mutex::new(outbound_handler)),
        )
    }
}
