use bytes::BytesMut;
use std::sync::Arc;
use tokio::net::{ToSocketAddrs, UdpSocket};

use crate::bootstrap::PipelineFactoryFn;
use crate::error::Error;

pub struct ServerBootstrapUdp {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
}

impl Default for ServerBootstrapUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerBootstrapUdp {
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
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

        tokio::spawn(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            while let Ok((n, _remote_addr)) = socket_rd.recv_from(&mut buf).await {
                //TODO: add cancellation handling
                if n == 0 {
                    pipeline.read_eof().await;
                    break;
                }
                let mut b = BytesMut::from(&buf[..n]);
                pipeline.read(&mut b).await;
            }
            pipeline.transport_inactive().await;
        });

        Ok(())
    }

    pub async fn stop(&self) {
        //TODO: add cancellation handling
    }
}
