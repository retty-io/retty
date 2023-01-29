//! Handlers for converting between (Tagged)BytesMut and (Tagged)String

mod tagged;

use async_trait::async_trait;
use bytes::{BufMut, BytesMut};

use crate::channel::{
    Handler, InboundHandler, InboundHandlerContext, OutboundHandler, OutboundHandlerContext,
};

pub use tagged::{TaggedString, TaggedStringCodec};

struct StringDecoder;
struct StringEncoder;

/// A StringCodec handler that reads with input of BytesMut and output of String,
/// or writes with input of String and output of BytesMut
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
    /// Creates a new StringCodec handler
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
        msg: Self::Rin,
    ) {
        match String::from_utf8(msg.to_vec()) {
            Ok(message) => {
                ctx.fire_read(message).await;
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
        msg: Self::Win,
    ) {
        let mut buf = BytesMut::new();
        buf.put(msg.as_bytes());
        ctx.fire_write(buf).await;
    }
}

impl Handler for StringCodec {
    type Rin = BytesMut;
    type Rout = String;
    type Win = String;
    type Wout = BytesMut;

    fn name(&self) -> &str {
        "StringCodec"
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
