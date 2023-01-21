use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION};
use crate::channel::pipeline::Pipeline;
use crate::error::Error;
use crate::runtime::{
    net::{ToSocketAddrs, UdpSocket},
    sleep, Runtime,
};
use crate::transport::async_transport_udp::TaggedBytesMut;
use crate::transport::TransportContext;

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
    pub async fn connect<A: ToSocketAddrs>(&mut self, addr: A) -> Result<Arc<Pipeline>, Error> {
        let socket = Arc::clone(self.socket.as_ref().unwrap());
        socket.connect(addr).await?;
        let (socket_rd, socket_wr) = (Arc::clone(&socket), socket);

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let async_writer = Box::new(socket_wr);
        let pipeline_wr = Arc::new((pipeline_factory_fn)(async_writer).await);

        let local_addr = socket_rd.local_addr()?;
        let pipeline = Arc::clone(&pipeline_wr);
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            loop {
                let mut timeout = Instant::now() + Duration::from_secs(MAX_DURATION);
                pipeline.poll_timeout(&mut timeout).await;

                let timer = if let Some(duration) = timeout.checked_duration_since(Instant::now()) {
                    sleep(duration)
                } else {
                    sleep(Duration::from_secs(0))
                };
                tokio::pin!(timer);

                tokio::select! {
                    _ = timer.as_mut() => {
                        pipeline.read_timeout(Instant::now()).await;
                    }
                    res = socket_rd.recv_from(&mut buf) => {
                        match res {
                            Ok((n, peer_addr)) => {
                                if n == 0 {
                                    pipeline.read_eof().await;
                                    break;
                                }

                                trace!("pipeline recv {} bytes", n);
                                pipeline
                                    .read(&mut TaggedBytesMut {
                                        transport: TransportContext {
                                            local_addr,
                                            peer_addr: Some(peer_addr),
                                        },
                                        message: BytesMut::from(&buf[..n]),
                                    })
                                    .await;
                            }
                            Err(err) => {
                                warn!("UdpSocket read error {}", err);
                                break;
                            }
                        };
                    }
                }
            }
            pipeline.transport_inactive().await;
        }));

        Ok(pipeline_wr)
    }
}
