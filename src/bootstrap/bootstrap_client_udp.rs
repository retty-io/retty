use bytes::BytesMut;
use local_sync::mpsc::unbounded::{channel, Rx, Tx};
use log::{trace, warn};
use monoio::{io::Canceller, net::udp::UdpSocket, time::sleep};
use std::{
    cell::RefCell,
    io::Error,
    net::{SocketAddr, ToSocketAddrs},
    rc::Rc,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::{InboundPipeline, OutboundPipeline};
use crate::transport::{TaggedBytesMut, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP clients.
pub struct BootstrapClientUdp<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<Tx<()>>>>,
    done_rx: Rc<RefCell<Option<Rx<()>>>>,
    socket: Option<Rc<UdpSocket>>,
}

impl<W: 'static> Default for BootstrapClientUdp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapClientUdp<W> {
    /// Creates a new BootstrapClientUdp
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Rc::new(RefCell::new(None)),
            done_rx: Rc::new(RefCell::new(None)),
            socket: None,
        }
    }

    /// Creates pipeline instances from when calling [BootstrapClientUdp::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let socket = Rc::new(UdpSocket::bind(addr)?);
        self.socket = Some(socket);
        Ok(())
    }

    /// Connects to the remote peer
    pub async fn connect(
        &mut self,
        addr: SocketAddr,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        let socket = Rc::clone(self.socket.as_ref().unwrap());
        socket.connect(addr).await?;
        let local_addr = socket.local_addr()?;

        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let (sender, mut receiver) = channel();
        let pipeline = (pipeline_factory_fn)(sender);
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

        monoio::spawn(async move {
            pipeline.transport_active();
            loop {
                if let Ok(transmit) = receiver.try_recv() {
                    let (res, _) = socket
                        .send_to(transmit.message, transmit.transport.peer_addr)
                        .await;
                    match res {
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

                let mut eto = Instant::now() + Duration::from_secs(MAX_DURATION_IN_SECS);
                pipeline.poll_timeout(&mut eto);

                let delay_from_now = eto
                    .checked_duration_since(Instant::now())
                    .unwrap_or(Duration::from_secs(0));
                if delay_from_now.is_zero() {
                    pipeline.handle_timeout(Instant::now());
                    continue;
                }

                let timeout = sleep(delay_from_now);
                let canceller = Canceller::new();

                monoio::select! {
                    _ = close_rx.recv() => {
                        canceller.cancel();
                        trace!("pipeline socket exit loop");
                        let _ = done_tx.send(());
                        break;
                    }
                    _ = timeout => {
                        canceller.cancel();
                        pipeline.handle_timeout(Instant::now());
                    }
                    opt = receiver.recv() => {
                        canceller.cancel();
                        if let Some(transmit) = opt {
                            let (res, _) = socket.send_to(transmit.message, transmit.transport.peer_addr).await;
                            match res {
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
                    (res, buf) = socket.cancelable_recv_from(Vec::with_capacity(1500), canceller.handle()) => {
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
        });

        Ok(pipeline_wr)
    }

    /// Gracefully stop the client
    pub async fn stop(&self) {
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
