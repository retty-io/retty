//! Handlers for converting byte to message
use bytes::BytesMut;
use sansio::{Context, Handler};
use sansio_transport::TaggedBytesMut;
use std::time::Instant;

mod line_based_frame_decoder;

pub use line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType};

/// This trait allows for decoding messages.
pub trait MessageDecoder {
    /// Decodes byte buffer to message buffer
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, std::io::Error>;
}

/// A tagged Byte to Message Codec handler that reads with input of TaggedBytesMut and output of TaggedBytesMut,
/// or writes with input of TaggedBytesMut and output of TaggedBytesMut
pub struct TaggedByteToMessageCodec {
    transport_active: bool,
    message_decoder: Box<dyn MessageDecoder + Send + Sync>,
}

impl TaggedByteToMessageCodec {
    /// Creates a new TaggedByteToMessageCodec handler
    pub fn new(message_decoder: Box<dyn MessageDecoder + Send + Sync>) -> Self {
        Self {
            transport_active: false,
            message_decoder,
        }
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

    fn transport_active(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        self.transport_active = true;
        ctx.fire_transport_active();
    }
    fn transport_inactive(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        self.transport_active = false;
        ctx.fire_transport_inactive();
    }
    fn handle_read(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        mut msg: Self::Rin,
    ) {
        while self.transport_active {
            match self.message_decoder.decode(&mut msg.message) {
                Ok(message) => {
                    if let Some(message) = message {
                        ctx.fire_handle_read(TaggedBytesMut {
                            now: Instant::now(),
                            transport: msg.transport,
                            message,
                        });
                    } else {
                        return;
                    }
                }
                Err(err) => {
                    ctx.fire_handle_error(Box::new(err));
                    return;
                }
            }
        }
    }

    fn poll_write(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout> {
        ctx.fire_poll_write()
    }
}
