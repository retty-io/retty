use bytes::BytesMut;
use log::trace;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWrite};

use crate::channel::pipeline::PipelineContext;
use crate::error::Error;

pub type PipelineFactoryFn = Box<
    dyn (Fn(
            Pin<Box<dyn AsyncWrite + Send + Sync>>,
        ) -> Pin<Box<dyn Future<Output = PipelineContext> + Send + 'static>>)
        + Send
        + Sync,
>;

pub struct ServerBootstrapTcp {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    stopped: Arc<AtomicBool>,
}

impl Default for ServerBootstrapTcp {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerBootstrapTcp {
    pub fn new() -> ServerBootstrapTcp {
        ServerBootstrapTcp {
            pipeline_factory_fn: None,
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// bind address and port
    pub async fn bind<A: tokio::net::ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        tokio::spawn(async move {
            while let Ok((socket, remote_addr)) = listener.accept().await {
                trace!("remote_addr {} connected", remote_addr);

                // A new task is spawned for each inbound socket. The socket is
                // moved to the new task and processed there.
                let child_pipeline_factory_fn = Arc::clone(&pipeline_factory_fn);
                tokio::spawn(async move {
                    ServerBootstrapTcp::process_pipeline(socket, child_pipeline_factory_fn).await;
                });
            }
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    async fn process_pipeline(
        socket: tokio::net::TcpStream,
        pipeline_factory_fn: Arc<PipelineFactoryFn>,
    ) {
        let mut buf = vec![0u8; 8196];
        let (mut socket_rd, socket_wr) = socket.into_split();
        let async_writer = Box::pin(socket_wr);
        let pipeline = (pipeline_factory_fn)(async_writer).await;

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
    }
}
