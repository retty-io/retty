use bytes::BytesMut;
use futures_lite::{AsyncReadExt, AsyncWriteExt};
use local_sync::mpsc::{unbounded::channel, unbounded::Tx as LocalSender};
use log::{trace, warn};
use smol::{Async, Task, Timer};
use std::{
    cell::RefCell,
    io::Error,
    net::TcpStream,
    rc::Rc,
    time::{Duration, Instant},
};
use waitgroup::WaitGroup;

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::{InboundPipeline, OutboundPipeline};
use crate::transport::{TaggedBytesMut, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP clients.
pub struct BootstrapTcpClient<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<LocalSender<()>>>>,
    wg: Rc<RefCell<Option<WaitGroup>>>,
}

impl<W: 'static> Default for BootstrapTcpClient<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapTcpClient<W> {
    /// Creates a new BootstrapTcpClient
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Rc::new(RefCell::new(None)),
            wg: Rc::new(RefCell::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapTcpClient::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Connects to the remote peer
    pub async fn connect<A: ToString>(
        &mut self,
        addr: A,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        let mut socket = Async::<TcpStream>::connect(addr).await?;
        let local_addr = socket.get_ref().local_addr()?;
        let peer_addr = socket.get_ref().peer_addr()?;

        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let (sender, mut receiver) = channel();
        let pipeline = (pipeline_factory_fn)(sender);
        let pipeline_wr = Rc::clone(&pipeline);

        let (close_tx, mut close_rx) = channel();
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
        })
        .detach();

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
        let wg = {
            let mut wg = self.wg.borrow_mut();
            wg.take()
        };
        if let Some(wg) = wg {
            wg.wait().await;
        }
    }
}
