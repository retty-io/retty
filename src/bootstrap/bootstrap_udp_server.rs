use bytes::BytesMut;
use std::sync::Arc;

use crate::bootstrap::PipelineFactoryFn;
use crate::error::Error;
use crate::runtime::net::{ToSocketAddrs, UdpSocket};
use crate::{Message, Runtime, TransportContext};

pub struct BootstrapUdpServer {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    runtime: Arc<dyn Runtime>,
}

impl BootstrapUdpServer {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
        }
    }

    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// bind address and port
    pub async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (socket_rd, socket_wr) = (Arc::clone(&socket), socket);
        let async_writer = Box::new(socket_wr);
        let pipeline = (pipeline_factory_fn)(async_writer).await;

        let local_addr = socket_rd.local_addr()?;
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            while let Ok((n, peer_addr)) = socket_rd.recv_from(&mut buf).await {
                //TODO: add cancellation handling
                if n == 0 {
                    pipeline.read_eof().await;
                    break;
                }

                pipeline
                    .read(Message {
                        transport: TransportContext {
                            local_addr,
                            peer_addr,
                        },
                        body: Box::new(BytesMut::from(&buf[..n])),
                    })
                    .await;
            }
            pipeline.transport_inactive().await;
        }));

        Ok(())
    }

    pub async fn stop(&self) {
        //TODO: add cancellation handling
    }
}
