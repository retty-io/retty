use async_trait::async_trait;
use async_transport::{Capabilities, Transmit};
use log::{trace, warn};
use std::io::ErrorKind;

use crate::channel::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler};
use crate::transport::{AsyncTransportWrite, TaggedBytesMut};

struct AsyncTransportUdpEcnDecoder;
struct AsyncTransportUdpEcnEncoder {
    writer: Option<Box<dyn AsyncTransportWrite + Send + Sync>>,
    capabilities: Capabilities,
}

/// Asynchronous UDP transport handler with ECN information that reads with input of TaggedBytesMut
/// and output of TaggedBytesMut, or writes with input of TaggedBytesMut and output of TaggedBytesMut
pub struct AsyncTransportUdpEcn {
    decoder: AsyncTransportUdpEcnDecoder,
    encoder: AsyncTransportUdpEcnEncoder,
}

impl AsyncTransportUdpEcn {
    /// Create a new asynchronous transport handler for UDP with ECN information
    pub fn new(writer: Box<dyn AsyncTransportWrite + Send + Sync>) -> Self {
        AsyncTransportUdpEcn {
            decoder: AsyncTransportUdpEcnDecoder {},
            encoder: AsyncTransportUdpEcnEncoder {
                writer: Some(writer),
                capabilities: Capabilities::new(),
            },
        }
    }
}

#[async_trait]
impl InboundHandler for AsyncTransportUdpEcnDecoder {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;

    async fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        ctx.fire_read(msg).await;
    }
}

#[async_trait]
impl OutboundHandler for AsyncTransportUdpEcnEncoder {
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        if let Some(writer) = &mut self.writer {
            if let Some(target) = msg.transport.peer_addr {
                let transmit = Transmit {
                    destination: target,
                    ecn: msg.ecn,
                    contents: msg.message.to_vec(),
                    segment_size: None,
                    src_ip: Some(msg.transport.local_addr.ip()),
                };
                match writer.send(&self.capabilities, &[transmit]).await {
                    Ok(n) => {
                        trace!(
                            "AsyncTransportUdpEcnEncoder --> write {} of {} bytes",
                            n,
                            msg.message.len()
                        );
                    }
                    Err(err) => {
                        warn!("AsyncTransportUdpEcnEncoder write error: {}", err);
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

impl Handler for AsyncTransportUdpEcn {
    type Rin = TaggedBytesMut;
    type Rout = Self::Rin;
    type Win = TaggedBytesMut;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "AsyncTransportUdpEcn"
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
