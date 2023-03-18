//! Handlers for converting byte to message
use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::transport::TaggedBytesMut;
use bytes::BytesMut;
use std::time::Instant;

mod line_based_frame_decoder;

pub use line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType};

/// This trait allows for decoding messages.
pub trait MessageDecoder {
    /// Decodes byte buffer to message buffer
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, std::io::Error>;
}

struct TaggedByteToMessageDecoder {
    transport_active: bool,
    message_decoder: Box<dyn MessageDecoder + Send + Sync>,
}

struct TaggedByteToMessageEncoder;

/// A tagged Byte to Message Codec handler that reads with input of TaggedBytesMut and output of TaggedBytesMut,
/// or writes with input of TaggedBytesMut and output of TaggedBytesMut
pub struct TaggedByteToMessageCodec {
    decoder: TaggedByteToMessageDecoder,
    encoder: TaggedByteToMessageEncoder,
}

impl TaggedByteToMessageCodec {
    /// Creates a new TaggedByteToMessageCodec handler
    pub fn new(message_decoder: Box<dyn MessageDecoder + Send + Sync>) -> Self {
        Self {
            decoder: TaggedByteToMessageDecoder {
                transport_active: false,
                message_decoder,
            },
            encoder: TaggedByteToMessageEncoder {},
        }
    }
}

impl InboundHandler for TaggedByteToMessageDecoder {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;

    fn transport_active(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        self.transport_active = true;
        ctx.fire_transport_active();
    }
    fn transport_inactive(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        self.transport_active = false;
        ctx.fire_transport_inactive();
    }
    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, mut msg: Self::Rin) {
        while self.transport_active {
            match self.message_decoder.decode(&mut msg.message) {
                Ok(message) => {
                    if let Some(message) = message {
                        ctx.fire_read(TaggedBytesMut {
                            now: Instant::now(),
                            transport: msg.transport,
                            message,
                        });
                    } else {
                        return;
                    }
                }
                Err(err) => {
                    ctx.fire_read_exception(Box::new(err));
                    return;
                }
            }
        }
    }
}

impl OutboundHandler for TaggedByteToMessageEncoder {
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg);
    }
}

impl Handler for TaggedByteToMessageCodec {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "TaggedByteToMessageCodec"
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
