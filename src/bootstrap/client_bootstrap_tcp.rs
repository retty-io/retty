use bytes::BytesMut;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

use crate::bootstrap::PipelineFactoryFn;
use crate::channel::pipeline::PipelineContext;
use crate::error::Error;

pub struct ClientBootstrapTcp {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
}

impl Default for ClientBootstrapTcp {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBootstrapTcp {
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
        }
    }

    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// connect host:port
    pub async fn connect<A: tokio::net::ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<PipelineContext>, Error> {
        let socket = tokio::net::TcpStream::connect(addr).await?;
        let (mut socket_rd, socket_wr) = socket.into_split();

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let async_writer = Box::pin(socket_wr);
        let pipeline_wr = Arc::new((pipeline_factory_fn)(async_writer).await);

        let pipeline = Arc::clone(&pipeline_wr);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            while let Ok(n) = socket_rd.read(&mut buf).await {
                if n == 0 {
                    pipeline.read_eof().await;
                    break;
                }
                let mut b = BytesMut::from(&buf[..n]);
                pipeline.read(&mut b).await;
            }
            pipeline.transport_inactive().await;
        });

        Ok(pipeline_wr)
    }
}
