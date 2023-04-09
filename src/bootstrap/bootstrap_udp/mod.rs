pub(crate) mod bootstrap_udp_client;
pub(crate) mod bootstrap_udp_server;

use async_transport::{AsyncUdpSocket, Capabilities, RecvMeta, Transmit, UdpSocket, BATCH_SIZE};
use bytes::BytesMut;
use local_sync::mpsc::{
    unbounded::channel, unbounded::Rx as LocalReceiver, unbounded::Tx as LocalSender,
};
use log::{trace, warn};
use smol::{net::AsyncToSocketAddrs, Timer};
use std::{
    cell::RefCell,
    io::Error,
    mem::MaybeUninit,
    net::SocketAddr,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::{InboundPipeline, OutboundPipeline};
use crate::executor::spawn_local;
use crate::transport::{AsyncTransportWrite, TaggedBytesMut, TransportContext};

struct BootstrapUdp<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    socket: Option<UdpSocket>,
    close_tx: Rc<RefCell<Option<LocalSender<()>>>>,
    done_rx: Rc<RefCell<Option<LocalReceiver<()>>>>,
}

impl<W: 'static> Default for BootstrapUdp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapUdp<W> {
    fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            socket: None,
            close_tx: Rc::new(RefCell::new(None)),
            done_rx: Rc::new(RefCell::new(None)),
        }
    }

    fn io_group(&mut self /*TODO: io_group: IOThreadPoolExecutor*/) -> &mut Self {
        self
    }

    fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    async fn bind<A: AsyncToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let socket = UdpSocket::bind(addr).await?;
        let local_addr = socket.local_addr()?;
        self.socket = Some(socket);
        Ok(local_addr)
    }

    async fn connect(
        &mut self,
        peer_addr: Option<SocketAddr>,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        let socket = self.socket.take().unwrap();
        let local_addr = socket.local_addr()?;

        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let (sender, mut receiver) = channel();
        let pipeline = (pipeline_factory_fn)(AsyncTransportWrite {
            sender,
            transport: TransportContext {
                local_addr,
                peer_addr,
                ecn: None,
            },
        });
        let pipeline_wr = Rc::clone(&pipeline);

        let (close_tx, mut close_rx) = channel();
        {
            let mut tx = self.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let (done_tx, done_rx) = channel();
        {
            let mut rx = self.done_rx.borrow_mut();
            *rx = Some(done_rx);
        }

        spawn_local(async move {
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

            pipeline.transport_active();
            loop {
                let mut eto = Instant::now() + Duration::from_secs(MAX_DURATION_IN_SECS);
                pipeline.poll_timeout(&mut eto);

                let delay_from_now = eto
                    .checked_duration_since(Instant::now())
                    .unwrap_or(Duration::from_secs(0));
                if delay_from_now.is_zero() {
                    pipeline.handle_timeout(Instant::now());
                    continue;
                }

                let timeout = Timer::after(delay_from_now);

                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("pipeline socket exit loop");
                        let _ = done_tx.send(());
                        break;
                    }
                    _ = timeout => {
                        pipeline.handle_timeout(Instant::now());
                    }
                    opt = receiver.recv() => {
                        if let Some(msg) = opt {
                            if let Some(peer_addr) = msg.transport.peer_addr {
                                let transmit = Transmit {
                                    destination: peer_addr,
                                    ecn: msg.transport.ecn,
                                    contents: msg.message.to_vec(),
                                    segment_size: None,
                                    src_ip: Some(msg.transport.local_addr.ip()),
                                };
                                match socket.send(&capabilities, &[transmit]).await {
                                    Ok(_) => {
                                        trace!("socket write {} bytes", msg.message.len());
                                    }
                                    Err(err) => {
                                        warn!("socket write error {}", err);
                                        break;
                                    }
                                }
                            } else {
                                warn!("socket write error due to peer_addr is missing");
                                break;
                            }
                        } else {
                            warn!("pipeline recv error");
                            break;
                        }
                    }
                    res = socket.recv(&mut iovs, &mut metas) => {
                        match res {
                            Ok(n) => {
                                if n == 0 {
                                    pipeline.read_eof();
                                    break;
                                }

                                for (meta, buf) in metas.iter().zip(iovs.iter()).take(n) {
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
                                            });
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("socket read error {}", err);
                                break;
                            }
                        }
                    }
                }
            }
            pipeline.transport_inactive();
        })
        .detach();

        Ok(pipeline_wr)
    }

    async fn stop(&self) {
        {
            let mut close_tx = self.close_tx.borrow_mut();
            if let Some(close_tx) = close_tx.take() {
                let _ = close_tx.send(());
            }
        }
        let done_rx = {
            let mut done_rx = self.done_rx.borrow_mut();
            done_rx.take()
        };
        if let Some(mut done_rx) = done_rx {
            let _ = done_rx.recv().await;
        }
    }
}
