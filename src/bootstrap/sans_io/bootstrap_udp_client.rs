use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::channel;

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION};
use crate::channel::Pipeline;
use crate::runtime::{
    mpsc::{bounded, Receiver, Sender},
    net::{ToSocketAddrs, UdpSocket},
    sleep,
    sync::Mutex,
    Runtime,
};
use crate::transport::{AsyncTransportRead, AsyncTransportWrite, TaggedBytesMut, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP clients.
pub struct BootstrapUdpClient<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    runtime: Arc<dyn Runtime>,
    socket: Option<Arc<UdpSocket>>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> BootstrapUdpClient<W> {
    /// Creates a new BootstrapUdpClient
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            socket: None,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapUdpClient::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error> {
        let socket = UdpSocket::bind(addr).await?;
        self.socket = Some(Arc::new(socket));
        Ok(())
    }

    /// Connects to the remote peer
    pub async fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<Pipeline<TaggedBytesMut, W>>, std::io::Error> {
        let socket = Arc::clone(self.socket.as_ref().unwrap());
        socket.connect(addr).await?;
        let (mut socket_rd, mut socket_wr) = (Arc::clone(&socket), socket);

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        #[allow(unused_mut)]
        let (close_tx, mut close_rx) = bounded(1);
        {
            let mut tx = self.close_tx.lock().await;
            *tx = Some(close_tx);
        }

        let (done_tx, done_rx) = bounded(1);
        let mut done_tx = Some(done_tx);
        {
            let mut rx = self.done_rx.lock().await;
            *rx = Some(done_rx);
        }

        let (sender, mut receiver) = channel(8); //TODO: make it configurable?
        let pipeline_wr = (pipeline_factory_fn)(sender).await;

        let local_addr = socket_rd.local_addr()?;
        let pipeline = Arc::clone(&pipeline_wr);
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            loop {
                if let Ok(transmit) = receiver.try_recv() {
                    match socket_wr
                        .write(&transmit.message, transmit.transport.peer_addr)
                        .await {
                        Ok(n) => {
                            trace!("socket write {} bytes", n);
                            continue;
                        }
                        Err(err) => {
                            warn!("socket write error {}", err);
                            break;
                        }
                    }
                }

                let mut eto = Instant::now() + Duration::from_secs(MAX_DURATION);
                pipeline.poll_timeout(&mut eto).await;

                let timer = if let Some(duration) = eto.checked_duration_since(Instant::now()) {
                    sleep(duration)
                } else {
                    sleep(Duration::from_secs(0))
                };
                tokio::pin!(timer);

                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("pipeline socket exit loop");
                        done_tx.take();
                        break;
                    }
                    _ = timer.as_mut() => {
                        pipeline.read_timeout(Instant::now()).await;
                    }
                    res = receiver.recv() => {
                        match res {
                            Ok(transmit) => {
                                match socket_wr.write(&transmit.message, transmit.transport.peer_addr).await {
                                    Ok(n) => {
                                        trace!("socket write {} bytes", n);
                                        continue;
                                    }
                                    Err(err) => {
                                        warn!("socket write error {}", err);
                                        break;
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("pipeline recv error {}", err);
                                break;
                            }
                        }
                    }
                    res = socket_rd.read(&mut buf) => {
                        match res {
                            Ok((n, peer_addr)) => {
                                if n == 0 {
                                    pipeline.read_eof().await;
                                    break;
                                }

                                trace!("socket read {} bytes", n);
                                pipeline
                                    .read(TaggedBytesMut {
                                        now: Instant::now(),
                                        transport: TransportContext {
                                            local_addr,
                                            peer_addr,
                                            ecn: None,
                                        },
                                        message: BytesMut::from(&buf[..n]),
                                    })
                                    .await;
                            }
                            Err(err) => {
                                warn!("socket read error {}", err);
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

    /// Gracefully stop the client
    pub async fn stop(&self) {
        {
            let mut tx = self.close_tx.lock().await;
            tx.take();
        }
        {
            let mut rx = self.done_rx.lock().await;
            #[allow(unused_mut)]
            if let Some(mut done_rx) = rx.take() {
                let _ = done_rx.recv().await;
            }
        }
    }
}
