use std::io::ErrorKind;
use std::sync::Arc;
use tokio::net::{ToSocketAddrs, UdpSocket};

use crate::bootstrap::PipelineFactoryFn;
use crate::channel::pipeline::PipelineContext;
use crate::error::Error;

pub struct ClientBootstrapUdp {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    socket: Option<Arc<UdpSocket>>,
}

impl Default for ClientBootstrapUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBootstrapUdp {
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            socket: None,
        }
    }

    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    pub async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> &mut Self {
        if let Ok(socket) = UdpSocket::bind(addr).await {
            self.socket = Some(Arc::new(socket));
        }
        self
    }

    /// connect host:port
    pub async fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<PipelineContext>, Error> {
        let socket = Arc::clone(self.socket.as_ref().unwrap());
        socket.connect(addr).await?;
        //let (socket_rd, socket_wr) = (socket.clone(), socket);

        /*let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let async_writer = Box::pin(socket);
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

        Ok(pipeline_wr)*/
        Err(Error::new(ErrorKind::Other, "TODO".to_string()))
    }
}
