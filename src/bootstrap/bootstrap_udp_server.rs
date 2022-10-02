use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;

use crate::bootstrap::PipelineFactoryFn;
use crate::error::Error;
use crate::runtime::{
    mpsc::{bounded, Sender},
    net::{ToSocketAddrs, UdpSocket},
    sync::Mutex,
};
use crate::{Message, Runtime, TransportContext};

pub struct BootstrapUdpServer {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    runtime: Arc<dyn Runtime>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
}

impl BootstrapUdpServer {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            close_tx: Arc::new(Mutex::new(None)),
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

        let (close_tx, mut close_rx) = bounded(1);
        {
            let mut tx = self.close_tx.lock().await;
            *tx = Some(close_tx);
        }

        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;

            loop {
                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("UdpSocket read exit loop");
                        break;
                    }
                    res = socket_rd.recv_from(&mut buf) => {
                        match res {
                            Ok((n, peer_addr)) => {
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

        Ok(())
    }

    pub async fn stop(&self) {
        let mut tx = self.close_tx.lock().await;
        tx.take();
    }
}
