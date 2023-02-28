use async_transport::{RecvMeta, BATCH_SIZE};
use bytes::BytesMut;
use log::{trace, warn};
use std::{
    mem::MaybeUninit,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::runtime::{
    mpsc::{bounded, Receiver, Sender},
    net::ToSocketAddrs,
    sleep,
    sync::Mutex,
    Runtime,
};
use crate::transport::{AsyncTransportRead, TaggedBytesMut, TransportAddress, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers with ECN information.
pub struct BootstrapServerUdpEcn<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    runtime: Arc<dyn Runtime>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> BootstrapServerUdpEcn<W> {
    /// Creates a new BootstrapServerUdpEcn
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapServerUdpEcn::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), std::io::Error> {
        let socket = Arc::new(async_transport::UdpSocket::bind(addr).await?);
        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (mut socket_rd, socket_wr) = (Arc::clone(&socket), socket);
        let async_writer = Box::new(socket_wr);
        let pipeline = (pipeline_factory_fn)(async_writer).await;

        let local_addr = socket_rd.local_addr()?;

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

        self.runtime.spawn(Box::pin(async move {
            let buf = vec![0u8; 8196];
            let buf_len = buf.len();
            let mut recv_buf: Box<[u8]> = buf.into();
            let mut metas = [RecvMeta::default(); BATCH_SIZE];
            let mut iovs = MaybeUninit::<[std::io::IoSliceMut<'_>; BATCH_SIZE]>::uninit();
            recv_buf
                .chunks_mut(buf_len / BATCH_SIZE)
                .enumerate()
                .for_each(|(i, buf)| unsafe {
                    iovs.as_mut_ptr()
                        .cast::<std::io::IoSliceMut<'_>>()
                        .add(i)
                        .write(std::io::IoSliceMut::<'_>::new(buf));
                });
            let mut iovs = unsafe { iovs.assume_init() };

            pipeline.transport_active().await;
            loop {
                let mut eto = Instant::now() + Duration::from_secs(MAX_DURATION_IN_SECS);
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
                    res = socket_rd.recv(&mut iovs, &mut metas) => {
                        match res {
                            Ok(msgs) => {
                                if msgs == 0 {
                                    pipeline.read_eof().await;
                                    break;
                                }

                                for (meta, buf) in metas.iter().zip(iovs.iter()).take(msgs) {
                                    let message: BytesMut = buf[0..meta.len].into();
                                    if !message.is_empty() {
                                        trace!("socket read {} bytes", message.len());
                                        pipeline
                                            .read(TaggedBytesMut {
                                                now: Instant::now(),
                                                transport: TransportContext {
                                                    local_addr,
                                                    peer_addr: Some(meta.addr),
                                                    ecn: meta.ecn,
                                                },
                                                message,
                                            })
                                            .await;
                                    }
                                }
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

        Ok(())
    }

    /// Gracefully stop the server
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
