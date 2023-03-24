pub(crate) mod bootstrap_tcp_client;
pub(crate) mod bootstrap_tcp_server;

use bytes::BytesMut;
use futures_lite::{AsyncReadExt, AsyncWriteExt};
use local_sync::mpsc::{unbounded::channel, unbounded::Rx as LocalReceiver};
use log::{trace, warn};
use smol::{Async, Task, Timer};
use std::net::SocketAddr;
use std::{
    cell::RefCell,
    io::Error,
    net::{TcpListener, TcpStream},
    rc::Rc,
    time::{Duration, Instant},
};
use waitgroup::{WaitGroup, Worker};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::{InboundPipeline, OutboundPipeline};
use crate::transport::{TaggedBytesMut, TransportContext};

struct BootstrapTcp<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<async_channel::Sender<()>>>>, //TODO: replace it with local_sync::broadcast channel
    wg: Rc<RefCell<Option<WaitGroup>>>,
}

impl<W: 'static> Default for BootstrapTcp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapTcp<W> {
    fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Rc::new(RefCell::new(None)),
            wg: Rc::new(RefCell::new(None)),
        }
    }

    fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    fn bind<A: ToString>(&self, addr: A) -> Result<SocketAddr, Error> {
        let listener = Async::<TcpListener>::bind(addr)?;
        let local_addr = listener.get_ref().local_addr()?;
        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, close_rx) = async_channel::bounded(1);
        {
            let mut tx = self.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let worker = {
            let workgroup = WaitGroup::new();
            let worker = workgroup.worker();
            {
                let mut wg = self.wg.borrow_mut();
                *wg = Some(workgroup);
            }
            worker
        };

        Task::local(async move {
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
                            Ok((socket,_)) => {
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let (sender, receiver) = channel();
                                let pipeline_rd = (pipeline_factory_fn)(sender);
                                let child_close_rx = close_rx.clone();
                                let child_worker = child_wg.worker();
                                Task::local(async move {
                                    let _ = Self::process_pipeline(socket,
                                                                   pipeline_rd,
                                                                   receiver,
                                                                   child_close_rx,
                                                                   child_worker).await;
                                }).detach();
                            }
                            Err(err) => {
                                warn!("listener accept error {}", err);
                                break;
                            }
                        }
                    }
                }
            }
            child_wg.wait().await;
        })
        .detach();

        Ok(local_addr)
    }

    async fn connect<A: ToString>(
        &mut self,
        addr: A,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        let socket = Async::<TcpStream>::connect(addr).await?;

        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, close_rx) = async_channel::bounded(1);
        {
            let mut tx = self.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let worker = {
            let workgroup = WaitGroup::new();
            let worker = workgroup.worker();
            {
                let mut wg = self.wg.borrow_mut();
                *wg = Some(workgroup);
            }
            worker
        };

        let (sender, receiver) = channel();
        let pipeline_rd = (pipeline_factory_fn)(sender);
        let pipeline_wr = Rc::clone(&pipeline_rd);
        Task::local(async move {
            let _ = Self::process_pipeline(socket, pipeline_rd, receiver, close_rx, worker).await;
        })
        .detach();

        Ok(pipeline_wr)
    }

    async fn process_pipeline(
        mut socket: Async<TcpStream>,
        pipeline: Rc<dyn InboundPipeline<TaggedBytesMut>>,
        mut receiver: LocalReceiver<TaggedBytesMut>,
        close_rx: async_channel::Receiver<()>,
        worker: Worker,
    ) -> Result<(), Error> {
        let _w = worker;

        let local_addr = socket.get_ref().local_addr()?;

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
                    break;
                }
                _ = timeout => {
                    pipeline.handle_timeout(Instant::now());
                }
                opt = receiver.recv() => {
                    if let Some(transmit) = opt {
                        match socket.write(&transmit.message).await {
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
                res = socket.read(&mut buf) => {
                    match res {
                        Ok(n) => {
                            if n == 0 {
                                pipeline.read_eof();
                                break;
                            }

                            trace!("socket read {} bytes", n);
                            pipeline.read(TaggedBytesMut {
                                    now: Instant::now(),
                                    transport: TransportContext {
                                        local_addr,
                                        peer_addr: None,
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

        Ok(())
    }

    async fn stop(&self) {
        {
            let mut close_tx = self.close_tx.borrow_mut();
            if let Some(close_tx) = close_tx.take() {
                let _ = close_tx.try_send(());
            }
        }
        let wg = {
            let mut wg = self.wg.borrow_mut();
            wg.take()
        };
        if let Some(wg) = wg {
            wg.wait().await;
        }
    }
}
