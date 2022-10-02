use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;

use crate::bootstrap::PipelineFactoryFn;
use crate::error::Error;
use crate::runtime::{
    io::AsyncReadExt,
    mpsc::{bounded, Receiver, Sender},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::Mutex,
};
use crate::{Message, Runtime, TransportContext};

pub struct BootstrapTcpServer {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    runtime: Arc<dyn Runtime>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
}

impl BootstrapTcpServer {
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
    pub async fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), Error> {
        let listener = TcpListener::bind(addr).await?;
        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, mut close_rx) = bounded(1);
        {
            let mut tx = self.close_tx.lock().await;
            *tx = Some(close_tx);
        }

        let close_tx = Arc::clone(&self.close_tx);
        let runtime = Arc::clone(&self.runtime);
        self.runtime.spawn(Box::pin(async move {
            loop {
                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("TcpStream accept exit loop");
                        break;
                    }

                    res = listener.accept() => {
                        match res {
                            Ok((socket, _remote_addr)) => {
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let child_pipeline_factory_fn = Arc::clone(&pipeline_factory_fn);
                                let child_close_rx = {
                                    let tx = close_tx.lock().await;
                                    if let Some(t) = &*tx {
                                        t.subscribe()
                                    } else {
                                        warn!("BootstrapTcpServer is closed");
                                        break
                                    }
                                };
                                runtime.spawn(Box::pin(async move {
                                    BootstrapTcpServer::process_pipeline(socket, child_pipeline_factory_fn, child_close_rx)
                                        .await;
                                }));
                            }
                            Err(err) => {
                                warn!("TcpListener accept error {}", err);
                                break;
                            }
                        };
                    }
                }
            }
        }));

        Ok(())
    }

    pub async fn stop(&mut self) {
        let mut tx = self.close_tx.lock().await;
        tx.take();
    }

    async fn process_pipeline(
        socket: TcpStream,
        pipeline_factory_fn: Arc<PipelineFactoryFn>,
        mut close_rx: Receiver<()>,
    ) {
        let mut buf = vec![0u8; 8196];

        #[cfg(feature = "runtime-tokio")]
        let (mut socket_rd, socket_wr) = socket.into_split();
        #[cfg(feature = "runtime-async-std")]
        let (mut socket_rd, socket_wr) = (socket.clone(), socket);

        let async_writer = Box::new(socket_wr);
        let pipeline = (pipeline_factory_fn)(async_writer).await;

        let transport = TransportContext {
            local_addr: socket_rd.local_addr().unwrap(),
            peer_addr: socket_rd.peer_addr().unwrap(),
        };

        pipeline.transport_active().await;
        loop {
            tokio::select! {
                _ = close_rx.recv() => {
                    trace!("TcpStream read exit loop");
                    break;
                }
                res = socket_rd.read(&mut buf) => {
                    match res {
                        Ok(n) => {
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
                        Err(err) => {
                            warn!("TcpStream read error {}", err);
                            break;
                        }
                    }
                }
            }
        }
        pipeline.transport_inactive().await;
    }
}
