use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;

use crate::channel::handler::*;
use crate::channel::handler_internal::{InboundHandlerInternal, OutboundHandlerInternal};
use crate::runtime::sync::Mutex;
use crate::transport::async_transport_udp::TaggedBytesMut;
use crate::transport::TransportContext;

struct TaggedStringDecoder;
struct TaggedStringEncoder;

pub struct TaggedStringCodec {
    decoder: TaggedStringDecoder,
    encoder: TaggedStringEncoder,
}

impl Default for TaggedStringCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl TaggedStringCodec {
    pub fn new() -> Self {
        TaggedStringCodec {
            decoder: TaggedStringDecoder {},
            encoder: TaggedStringEncoder {},
        }
    }
}

#[derive(Default)]
pub struct TaggedString {
    pub transport: TransportContext,
    pub message: String,
}

#[async_trait]
impl InboundHandler for TaggedStringDecoder {
    type Rin = TaggedBytesMut;
    type Rout = TaggedString;

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        msg: &mut Self::Rin,
    ) {
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
impl OutboundHandler for TaggedStringEncoder {
    type Win = TaggedString;
    type Wout = TaggedBytesMut;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        msg: &mut Self::Win,
    ) {
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
    type In = TaggedBytesMut;
    type Out = TaggedString;

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
