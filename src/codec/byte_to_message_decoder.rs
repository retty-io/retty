use async_trait::async_trait;
use bytes::BytesMut;
use std::any::Any;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::channel::handler::*;
use crate::error::Error;

pub trait MessageDecoder {
    fn id(&self) -> String;
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, Error>;
}

struct ByteToMessageDecoder {
    transport_active: bool,
    message_decoder: Box<dyn MessageDecoder + Send + Sync>,
}

struct ByteToMessageEncoder;

pub struct ByteToMessageCodec {
    decoder: ByteToMessageDecoder,
    encoder: ByteToMessageEncoder,
}

impl ByteToMessageCodec {
    pub fn new(message_decoder: Box<dyn MessageDecoder + Send + Sync>) -> Self {
        Self {
            decoder: ByteToMessageDecoder {
                transport_active: false,
                message_decoder,
            },
            encoder: ByteToMessageEncoder {},
        }
    }
}

#[async_trait]
impl InboundHandler for ByteToMessageDecoder {
    async fn transport_active(&mut self, ctx: &mut InboundHandlerContext) {
        self.transport_active = true;
        ctx.fire_transport_active().await;
    }
    async fn transport_inactive(&mut self, ctx: &mut InboundHandlerContext) {
        self.transport_active = false;
        ctx.fire_transport_inactive().await;
    }

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext,
        message: &mut (dyn Any + Send + Sync),
    ) {
        if let Some(buf) = message.downcast_mut::<BytesMut>() {
            while self.transport_active {
                match self.message_decoder.decode(buf) {
                    Ok(msg) => {
                        if let Some(mut msg) = msg {
                            ctx.fire_read(&mut msg).await;
                        } else {
                            return;
                        }
                    }
                    Err(err) => {
                        ctx.fire_read_exception(err).await;
                        return;
                    }
                }
            }
        } else {
            let err = Error::new(ErrorKind::Other, String::from("decoding error"));
            ctx.fire_read_exception(err).await;
        }
    }
}

#[async_trait]
impl OutboundHandler for ByteToMessageEncoder {}

impl Handler for ByteToMessageCodec {
    fn id(&self) -> String {
        self.decoder.message_decoder.id()
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
