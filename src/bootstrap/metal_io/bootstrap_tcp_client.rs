use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::channel;

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION};
use crate::channel::Pipeline;
use crate::runtime::{
    mpsc::{bounded, Receiver, Sender},
    net::{TcpStream, ToSocketAddrs},
    sleep,
    sync::Mutex,
    Runtime,
};
use crate::transport::{AsyncTransportRead, AsyncTransportWrite};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP clients.
pub struct BootstrapTcpClient<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<BytesMut, W>>>,
    runtime: Arc<dyn Runtime>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> BootstrapTcpClient<W> {
    /// Creates a new BootstrapTcpClient
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapTcpClient::connect].
    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<BytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Connects to the remote peer
    pub async fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<Pipeline<BytesMut, W>>, std::io::Error> {
        let socket = TcpStream::connect(addr).await?;

        #[cfg(feature = "runtime-tokio")]
        let (mut socket_rd, mut socket_wr) = socket.into_split();
        #[cfg(feature = "runtime-async-std")]
        let (mut socket_rd, mut socket_wr) = (socket.clone(), socket);

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

        let pipeline = Arc::clone(&pipeline_wr);
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            loop {
                if let Ok(transmit) = receiver.try_recv() {
                    match socket_wr.write(&transmit, None).await {
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
                        pipeline.handle_timeout(Instant::now()).await;
                    }
                    res = receiver.recv() => {
                        match res {
                            Ok(transmit) => {
                                match socket_wr.write(&transmit, None).await {
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
                            Ok((n,_)) => {
                                if n == 0 {
                                    pipeline.read_eof().await;
                                    break;
                                }

                                trace!("socket read {} bytes", n);
                                pipeline.read(BytesMut::from(&buf[..n])).await;
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
    pub async fn stop(&mut self) {
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
