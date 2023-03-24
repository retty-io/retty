use bytes::BytesMut;
use local_sync::mpsc::{
    unbounded::channel, unbounded::Rx as LocalReceiver, unbounded::Tx as LocalSender,
};
use log::{trace, warn};
use smol::{Async, Task, Timer};
use std::{
    cell::RefCell,
    io::{Error, ErrorKind},
    net::{SocketAddr, UdpSocket},
    rc::Rc,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::{InboundPipeline, OutboundPipeline};
use crate::transport::{TaggedBytesMut, TransportContext};

pub(crate) mod bootstrap_udp_client;
pub(crate) mod bootstrap_udp_server;

struct BootstrapUdp<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<LocalSender<()>>>>,
    done_rx: Rc<RefCell<Option<LocalReceiver<()>>>>,
    socket: Option<Rc<Async<UdpSocket>>>,
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
            close_tx: Rc::new(RefCell::new(None)),
            done_rx: Rc::new(RefCell::new(None)),
            socket: None,
        }
    }

    fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>) {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
    }

    fn bind<A: ToString>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let socket = Async::<UdpSocket>::bind(addr)?;
        let local_addr = socket.get_ref().local_addr()?;
        self.socket = Some(Rc::new(socket));
        Ok(local_addr)
    }

    fn serve(&mut self) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        let socket = Rc::clone(self.socket.as_ref().ok_or(Error::new(
            ErrorKind::AddrNotAvailable,
            "socket is not bind yet",
        ))?);
        let local_addr = socket.get_ref().local_addr()?;

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

        Task::local(async move {
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