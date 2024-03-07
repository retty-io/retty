//! Handlers for converting between TaggedBytesMut and TaggedString

use bytes::{BufMut, BytesMut};
use std::time::Instant;

use crate::channel::{Context, Handler};
use crate::transport::{TaggedBytesMut, TransportContext};

/// A tagged StringCodec handler that reads with input of TaggedBytesMut and output of TaggedString,
/// or writes with input of TaggedString and output of TaggedBytesMut
pub struct TaggedStringCodec;

impl Default for TaggedStringCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl TaggedStringCodec {
    /// Creates a new TaggedStringCodec handler
    pub fn new() -> Self {
        Self {}
    }
}

/// A tagged String with [TransportContext]
#[derive(Clone)]
pub struct TaggedString {
    /// Received/Sent time
    pub now: Instant,
    /// A transport context with [local_addr](TransportContext::local_addr) and [peer_addr](TransportContext::peer_addr)
    pub transport: TransportContext,
    /// Message body with String type
    pub message: String,
}

impl Default for TaggedString {
    fn default() -> Self {
        Self {
            now: Instant::now(),
            transport: TransportContext::default(),
            message: String::new(),
        }
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

    fn handle_read(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    ) {
        match String::from_utf8(msg.message.to_vec()) {
            Ok(message) => {
                ctx.fire_read(TaggedString {
                    now: msg.now,
                    transport: msg.transport,
                    message,
                });
            }
            Err(err) => ctx.fire_exception(err.into()),
        }
    }

    fn poll_write(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout> {
        if let Some(msg) = ctx.fire_poll_write() {
            let mut buf = BytesMut::new();
            buf.put(msg.message.as_bytes());
            Some(TaggedBytesMut {
                now: Instant::now(),
                transport: msg.transport,
                message: buf,
            })
        } else {
            None
        }
    }
}
