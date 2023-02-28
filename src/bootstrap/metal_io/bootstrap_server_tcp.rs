use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::channel;
use waitgroup::{WaitGroup, Worker};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION};
use crate::runtime::{
    mpsc::{bounded, Receiver, Sender},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sleep,
    sync::Mutex,
    Runtime,
};
use crate::transport::{AsyncTransportRead, AsyncTransportWrite};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapTcpServer<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<BytesMut, W>>>,
    runtime: Arc<dyn Runtime>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    wg: Arc<Mutex<Option<WaitGroup>>>,
}

impl<W: Send + Sync + 'static> BootstrapTcpServer<W> {
    /// Creates a new BootstrapTcpServer
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            close_tx: Arc::new(Mutex::new(None)),
            wg: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapTcpServer::bind].
    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<BytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub async fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind(addr).await?;
        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        #[allow(unused_mut)]
        let (close_tx, mut close_rx) = bounded(1);
        {
            let mut tx = self.close_tx.lock().await;
            *tx = Some(close_tx);
        }

        #[cfg(feature = "runtime-tokio")]
        let close_tx = Arc::clone(&self.close_tx);

        let worker = {
            let workgroup = WaitGroup::new();
            let worker = workgroup.worker();
            {
                let mut wg = self.wg.lock().await;
                *wg = Some(workgroup);
            }
            worker
        };

        let runtime = Arc::clone(&self.runtime);
        self.runtime.spawn(Box::pin(async move {
            let _w = worker;
            let child_wg = WaitGroup::new();

            loop {
                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("listener exit loop");
                        break;
                    }

                    res = listener.accept() => {
                        match res {
                            Ok((socket, _remote_addr)) => {
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let child_pipeline_factory_fn = Arc::clone(&pipeline_factory_fn);

                                #[cfg(feature = "runtime-tokio")]
                                let child_close_rx = {
                                    let tx = close_tx.lock().await;
                                    if let Some(t) = &*tx {
                                        t.subscribe()
                                    } else {
                                        warn!("BootstrapServerTcp is closed");
                                        break
                                    }
                                };
                                #[cfg(feature = "runtime-async-std")]
                                let child_close_rx = {
                                    close_rx.clone()
                                };

                                let child_worker = child_wg.worker();
                                runtime.spawn(Box::pin(async move {
                                    BootstrapTcpServer::process_pipeline(socket, child_pipeline_factory_fn, child_close_rx, child_worker)
                                        .await;
                                }));
                            }
                            Err(err) => {
                                warn!("listener accept error {}", err);
                                break;
                            }
                        };
                    }
                }
            }
            child_wg.wait().await;
        }));

        Ok(())
    }

    async fn process_pipeline(
        socket: TcpStream,
        pipeline_factory_fn: Arc<PipelineFactoryFn<BytesMut, W>>,
        #[allow(unused_mut)] mut close_rx: Receiver<()>,
        worker: Worker,
    ) {
        let _w = worker;
        let mut buf = vec![0u8; 8196];

        #[cfg(feature = "runtime-tokio")]
        let (mut socket_rd, mut socket_wr) = socket.into_split();
        #[cfg(feature = "runtime-async-std")]
        let (mut socket_rd, mut socket_wr) = (socket.clone(), socket);

        let (sender, mut receiver) = channel(8); //TODO: make it configurable?
        let pipeline = (pipeline_factory_fn)(sender).await;

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
    }

    /// Gracefully stop the server
    pub async fn stop(&mut self) {
        {
            let mut tx = self.close_tx.lock().await;
            tx.take();
        }
        {
            let mut wg = self.wg.lock().await;
            if let Some(wg) = wg.take() {
                wg.wait().await;
            }
        }
    }
}
