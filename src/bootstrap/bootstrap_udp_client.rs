use bytes::BytesMut;
use std::sync::Arc;

use crate::bootstrap::PipelineFactoryFn;
use crate::channel::pipeline::PipelineContext;
use crate::error::Error;
use crate::runtime::{
    net::{ToSocketAddrs, UdpSocket},
    Runtime,
};

pub struct BootstrapUdpClient {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    runtime: Arc<dyn Runtime>,
    socket: Option<Arc<UdpSocket>>,
}

impl BootstrapUdpClient {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            socket: None,
        }
    }

    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    pub async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let socket = UdpSocket::bind(addr).await?;
        self.socket = Some(Arc::new(socket));
        Ok(())
    }

    /// connect host:port
    pub async fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<PipelineContext>, Error> {
        let socket = Arc::clone(self.socket.as_ref().unwrap());
        socket.connect(addr).await?;
        let (socket_rd, socket_wr) = (Arc::clone(&socket), socket);

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let async_writer = Box::new(socket_wr);
        let pipeline_wr = Arc::new((pipeline_factory_fn)(async_writer).await);

        let _local_addr = socket_rd.local_addr()?;
        let pipeline = Arc::clone(&pipeline_wr);
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            while let Ok((n, _peer_addr)) = socket_rd.recv_from(&mut buf).await {
                if n == 0 {
                    pipeline.read_eof().await;
                    break;
                }

                pipeline.read(&mut BytesMut::from(&buf[..n])).await;
            }
            pipeline.transport_inactive().await;
        }));

        Ok(pipeline_wr)
    }
}
