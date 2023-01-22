use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use std::sync::Arc;

use crate::channel::{
    Handler, InboundHandler, InboundHandlerContext, InboundHandlerInternal, OutboundHandler,
    OutboundHandlerContext, OutboundHandlerInternal,
};
use crate::runtime::sync::Mutex;
use crate::transport::{TaggedBytesMut, TransportContext};

struct TaggedStringDecoder;
struct TaggedStringEncoder;

/// A tagged StringCodec handler that reads input of TaggedBytesMut and output of TaggedString,
/// or writes input of TaggedString and output of TaggedBytesMut
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
    /// Creates a new TaggedStringCodec handler
    pub fn new() -> Self {
        TaggedStringCodec {
            decoder: TaggedStringDecoder {},
            encoder: TaggedStringEncoder {},
        }
    }
}

/// A tagged String with [TransportContext]
#[derive(Default)]
pub struct TaggedString {
    /// A transport context with [local_addr](TransportContext::local_addr) and [peer_addr](TransportContext::peer_addr)
    pub transport: TransportContext,
    /// Message body with String type
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

    fn name(&self) -> &str {
        "TaggedStringCodec"
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
