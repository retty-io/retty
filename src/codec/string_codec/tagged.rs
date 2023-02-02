use async_trait::async_trait;
use bytes::{BufMut, BytesMut};

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::transport::{TaggedBytesMut, TransportContext};

struct TaggedStringDecoder;
struct TaggedStringEncoder;

/// A tagged StringCodec handler that reads with input of TaggedBytesMut and output of TaggedString,
/// or writes with input of TaggedString and output of TaggedBytesMut
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

    async fn read(&mut self, ctx: &mut InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        match String::from_utf8(msg.message.to_vec()) {
            Ok(message) => {
                ctx.fire_read(TaggedString {
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

    async fn write(&mut self, ctx: &mut OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        let mut buf = BytesMut::new();
        buf.put(msg.message.as_bytes());
        ctx.fire_write(TaggedBytesMut {
            transport: msg.transport,
            message: buf,
        })
        .await;
    }
}

impl Handler for TaggedStringCodec {
    type Rin = TaggedBytesMut;
    type Rout = TaggedString;
    type Win = TaggedString;
    type Wout = TaggedBytesMut;

    fn name(&self) -> &str {
        "TaggedStringCodec"
    }

    fn split(
        self,
    ) -> (
        Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
        Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
    ) {
        (Box::new(self.decoder), Box::new(self.encoder))
    }
}
