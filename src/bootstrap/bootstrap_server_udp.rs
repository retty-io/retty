use bytes::BytesMut;
use glommio::{
    channels::local_channel::{new_unbounded, LocalReceiver, LocalSender},
    {net::UdpSocket, timer::Timer},
};
use log::{trace, warn};
use std::{
    cell::RefCell,
    io::Error,
    net::{SocketAddr, ToSocketAddrs},
    rc::Rc,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::InboundPipeline;
use crate::transport::{TaggedBytesMut, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers.
pub struct BootstrapServerUdp<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<LocalSender<()>>>>,
    done_rx: Rc<RefCell<Option<LocalReceiver<()>>>>,
}

impl<W: 'static> Default for BootstrapServerUdp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapServerUdp<W> {
    /// Creates a new BootstrapServerUdp
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Rc::new(RefCell::new(None)),
            done_rx: Rc::new(RefCell::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapServerUdp::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port, return local socket address
    pub fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let socket = UdpSocket::bind(addr)?;
        let local_addr = socket.local_addr()?;

        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let (sender, receiver) = new_unbounded();
        let pipeline = (pipeline_factory_fn)(sender);

        let (close_tx, close_rx) = new_unbounded();
        {
            let mut tx = self.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let (done_tx, done_rx) = new_unbounded();
        {
            let mut rx = self.done_rx.borrow_mut();
            *rx = Some(done_rx);
        }

        glommio::spawn_local(async move {
            let mut buf = vec![0u8; 2048];

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

                let timeout = Timer::new(delay_from_now);

                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("pipeline socket exit loop");
                        let _ = done_tx.try_send(());
                        break;
                    }
                    _ = timeout => {
                        pipeline.handle_timeout(Instant::now());
                    }
                    opt = receiver.recv() => {
                        if let Some(transmit) = opt {
                            match socket.send_to(&transmit.message, transmit.transport.peer_addr).await {
                                Ok(n) => {
                                    trace!("socket write {} bytes", n);
                                }
                                Err(err) => {
                                    warn!("socket write error {}", err);
                                    break;
                                }
                            }
                        } else {
                            warn!("pipeline recv error");
                            break;
                        }
                    }
                    res = socket.recv_from(&mut buf) => {
                        match res {
                            Ok((n, peer_addr)) => {
                                if n == 0 {
                                    pipeline.read_eof();
                                    break;
                                }

                                trace!("socket read {} bytes", n);
                                pipeline.read(TaggedBytesMut {
                                    now: Instant::now(),
                                    transport: TransportContext {
                                        local_addr,
                                        peer_addr,
                                        ecn: None,
                                    },
                                    message: BytesMut::from(&buf[..n]),
                                });
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
        }).detach();

        Ok(local_addr)
    }

    /// Gracefully stop the server
    pub async fn stop(&self) {
        {
            let mut close_tx = self.close_tx.borrow_mut();
            if let Some(close_tx) = close_tx.take() {
                let _ = close_tx.try_send(());
            }
        }
        let done_rx = {
            let mut done_rx = self.done_rx.borrow_mut();
            done_rx.take()
        };
        if let Some(done_rx) = done_rx {
            let _ = done_rx.recv().await;
        }
    }
}
