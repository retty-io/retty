use async_transport::{Capabilities, RecvMeta, Transmit, BATCH_SIZE};
use bytes::BytesMut;
use log::{trace, warn};
use std::{
    mem::MaybeUninit,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::broadcast::channel;

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION};
use crate::runtime::{
    mpsc::{bounded, Receiver, Sender},
    net::ToSocketAddrs,
    sleep,
    sync::Mutex,
    Runtime,
};
use crate::transport::{
    AsyncTransportRead, AsyncTransportWrite, TaggedBytesMut, TransportAddress, TransportContext,
};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers with ECN information.
pub struct BootstrapUdpEcnServer<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    runtime: Arc<dyn Runtime>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> BootstrapUdpEcnServer<W> {
    /// Creates a new BootstrapUdpEcnServer
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapUdpEcnServer::bind].
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

        let (mut socket_rd, mut socket_wr) = (Arc::clone(&socket), socket);
        let (sender, mut receiver) = channel(8); //TODO: make it configurable?
        let pipeline = (pipeline_factory_fn)(sender).await;

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
            let capabilities = Capabilities::new();

            pipeline.transport_active().await;
            loop {
                if let Ok(msg) = receiver.try_recv() {
                    if let Some(target) = msg.transport.peer_addr {
                        let transmit = Transmit {
                            destination: target,
                            ecn: msg.transport.ecn,
                            contents: msg.message.to_vec(),
                            segment_size: None,
                            src_ip: Some(msg.transport.local_addr.ip()),
                        };
                        match socket_wr.send(&capabilities, &[transmit]).await {
                            Ok(n) => {
                                trace!("socket write {} bytes", n);
                                continue;
                            }
                            Err(err) => {
                                warn!("socket write error {}", err);
                                break;
                            }
                        }
                    } else {
                        trace!("socket can't write due to none peer_addr");
                        continue;
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
                            Ok(msg) => {
                                if let Some(target) = msg.transport.peer_addr {
                                    let transmit = Transmit {
                                        destination: target,
                                        ecn: msg.transport.ecn,
                                        contents: msg.message.to_vec(),
                                        segment_size: None,
                                        src_ip: Some(msg.transport.local_addr.ip()),
                                    };
                                    match socket_wr.send(&capabilities, &[transmit]).await {
                                        Ok(n) => {
                                            trace!("socket write {} bytes", n);
                                            continue;
                                        }
                                        Err(err) => {
                                            warn!("socket write error {}", err);
                                            break;
                                        }
                                    }
                                } else {
                                    trace!("socket can't write due to none peer_addr");
                                }
                            }
                            Err(err) => {
                                warn!("pipeline recv error {}", err);
                                break;
                            }
                        }
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
