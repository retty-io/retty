use bytes::BytesMut;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, ToSocketAddrs, UdpSocket};

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
        let _socket = UdpSocket::bind(addr).await?;
        let _pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        /*tokio::spawn(async move {
            while let Ok((socket, remote_addr)) = listener.accept().await {
                trace!("remote_addr {} connected", remote_addr);

                // A new task is spawned for each inbound socket. The socket is
                // moved to the new task and processed there.
                let child_pipeline_factory_fn = Arc::clone(&pipeline_factory_fn);
                tokio::spawn(async move {
                    ServerBootstrapUdp::process_pipeline(socket, child_pipeline_factory_fn).await;
                });
            }
        });*/

        Ok(())
    }

    pub async fn stop(&self) {
        //TODO: add cancellation handling
    }

    async fn process_pipeline(socket: TcpStream, pipeline_factory_fn: Arc<PipelineFactoryFn>) {
        let mut buf = vec![0u8; 8196];
        let (mut socket_rd, socket_wr) = socket.into_split();
        let async_writer = Box::new(socket_wr);
        let pipeline = (pipeline_factory_fn)(async_writer).await;

        pipeline.transport_active().await;
        while let Ok(n) = socket_rd.read(&mut buf).await {
            //TODO: add cancellation handling
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
