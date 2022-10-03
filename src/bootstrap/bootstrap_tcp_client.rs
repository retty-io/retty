use bytes::BytesMut;
use std::sync::Arc;

use crate::bootstrap::PipelineFactoryFn;
use crate::channel::pipeline::PipelineContext;
use crate::error::Error;
use crate::runtime::io::AsyncReadExt;
use crate::runtime::net::{TcpStream, ToSocketAddrs};
use crate::{Message, Runtime, TransportContext};

pub struct BootstrapTcpClient {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    runtime: Arc<dyn Runtime>,
}

impl BootstrapTcpClient {
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

    /// connect host:port
    pub async fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<PipelineContext>, Error> {
        let socket = TcpStream::connect(addr).await?;

        #[cfg(feature = "runtime-tokio")]
        let (mut socket_rd, socket_wr) = socket.into_split();
        #[cfg(feature = "runtime-async-std")]
        let (mut socket_rd, socket_wr) = (socket.clone(), socket);

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let async_writer = Box::new(socket_wr);
        let pipeline_wr = Arc::new((pipeline_factory_fn)(async_writer).await);

        let transport = TransportContext {
            local_addr: socket_rd.local_addr()?,
            peer_addr: Some(socket_rd.peer_addr()?),
        };
        let pipeline = Arc::clone(&pipeline_wr);
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            while let Ok(n) = socket_rd.read(&mut buf).await {
                if n == 0 {
                    pipeline.read_eof().await;
                    break;
                }

                pipeline
                    .read(Message {
                        transport,
                        body: Box::new(BytesMut::from(&buf[..n])),
                    })
                    .await;
            }
            pipeline.transport_inactive().await;
        }));

        Ok(pipeline_wr)
    }
}
