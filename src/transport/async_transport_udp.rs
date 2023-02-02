use async_trait::async_trait;
use bytes::BytesMut;
use log::{trace, warn};
use std::io::ErrorKind;

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::transport::{AsyncTransportWrite, TransportContext};

struct AsyncTransportUdpDecoder;
struct AsyncTransportUdpEncoder {
    writer: Option<Box<dyn AsyncTransportWrite + Send + Sync>>,
}

/// Asynchronous UDP transport handler for  that reads with input of TaggedBytesMut and output of TaggedBytesMut,
/// or writes with input of TaggedBytesMut and output of TaggedBytesMut
pub struct AsyncTransportUdp {
    decoder: AsyncTransportUdpDecoder,
    encoder: AsyncTransportUdpEncoder,
}

impl AsyncTransportUdp {
    /// Create a new asynchronous transport handler for UDP
    pub fn new(writer: Box<dyn AsyncTransportWrite + Send + Sync>) -> Self {
        AsyncTransportUdp {
            decoder: AsyncTransportUdpDecoder {},
            encoder: AsyncTransportUdpEncoder {
                writer: Some(writer),
            },
        }
    }
}

/// A tagged [BytesMut](bytes::BytesMut) with [TransportContext]
#[derive(Default)]
pub struct TaggedBytesMut {
    /// A transport context with [local_addr](TransportContext::local_addr) and [peer_addr](TransportContext::peer_addr)
    pub transport: TransportContext,
    /// Message body with [BytesMut](bytes::BytesMut) type
    pub message: BytesMut,
}

#[async_trait]
impl InboundHandler for AsyncTransportUdpDecoder {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;

    async fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        ctx.fire_read(msg).await;
    }
}

#[async_trait]
impl OutboundHandler for AsyncTransportUdpEncoder {
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        if let Some(writer) = &mut self.writer {
            if let Some(target) = msg.transport.peer_addr {
                match writer.write(&msg.message, Some(target)).await {
                    Ok(n) => {
                        trace!(
                            "AsyncTransportUdpEncoder --> write {} of {} bytes",
                            n,
                            msg.message.len()
                        );
                    }
                    Err(err) => {
                        warn!("AsyncTransportUdpEncoder write error: {}", err);
                        ctx.fire_write_exception(err.into()).await;
                    }
                }
            } else {
                let err = Box::new(std::io::Error::new(
                    ErrorKind::NotConnected,
                    String::from("Transport endpoint is not connected"),
                ));
                ctx.fire_write_exception(err).await;
            }
        }
    }
    async fn close(&mut self, _ctx: &OutboundContext<Self::Win, Self::Wout>) {
        trace!("close socket");
        self.writer.take();
    }
}

impl Handler for AsyncTransportUdp {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "AsyncTransportUdp"
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
