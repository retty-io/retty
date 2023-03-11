use bytes::BytesMut;
use local_sync::mpsc::unbounded::{channel, Rx, Tx};
use log::{trace, warn};
use monoio::{
    io::{AsyncWriteRent, CancelableAsyncReadRent, Canceller},
    net::tcp::{TcpListener, TcpStream},
    time::sleep,
};
use std::{
    cell::RefCell,
    io::Error,
    net::ToSocketAddrs,
    rc::Rc,
    time::{Duration, Instant},
};
use waitgroup::{WaitGroup, Worker};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::InboundPipeline;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapServerTcp<W> {
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<BytesMut, W>>>,
    close_tx: Rc<RefCell<Option<Tx<()>>>>,
    wg: Rc<RefCell<Option<WaitGroup>>>,
}

impl<W: 'static> Default for BootstrapServerTcp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapServerTcp<W> {
    /// Creates a new BootstrapServerTcp
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Rc::new(RefCell::new(None)),
            wg: Rc::new(RefCell::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapServerTcp::bind].
    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<BytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), Error> {
        let listener = TcpListener::bind(addr)?;
        let pipeline_factory_fn = Rc::clone(self.pipeline_factory_fn.as_ref().unwrap());

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

        monoio::spawn(async move {
            let _w = worker;
            let child_wg = WaitGroup::new();

            //TODO: https://github.com/monoio-rs/local-sync/issues/3
            let mut broadcast_close = vec![];

            loop {
                monoio::select! {
                    _ = close_rx.recv() => {
                        trace!("listener exit loop");
                        break;
                    }
                    res = listener.accept() => {
                        match res {
                            Ok((socket, _remote_addr)) => {
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let child_pipeline_factory_fn = Rc::clone(&pipeline_factory_fn);
                                let (child_close_tx, child_close_rx) = channel();
                                broadcast_close.push(child_close_tx);
                                let child_worker = child_wg.worker();
                                monoio::spawn(async move {
                                    Self::process_pipeline(socket, child_pipeline_factory_fn, child_close_rx, child_worker)
                                        .await;
                                });
                            }
                            Err(err) => {
                                warn!("listener accept error {}", err);
                                break;
                            }
                        }
                    }
                }
            }
            //TODO: https://github.com/monoio-rs/local-sync/issues/3
            for child_close_tx in broadcast_close {
                let _ = child_close_tx.send(());
            }
            child_wg.wait().await;
        });

        Ok(())
    }

    async fn process_pipeline(
        mut socket: TcpStream,
        pipeline_factory_fn: Rc<PipelineFactoryFn<BytesMut, W>>,
        mut close_rx: Rx<()>,
        worker: Worker,
    ) {
        let _w = worker;

        let (sender, mut receiver) = channel();
        let pipeline = (pipeline_factory_fn)(sender);

        pipeline.transport_active();
        loop {
            if let Ok(transmit) = receiver.try_recv() {
                let (res, _) = socket.write(transmit).await;
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
                    break;
                }
                _ = timeout => {
                    canceller.cancel();
                    pipeline.handle_timeout(Instant::now());
                }
                opt = receiver.recv() => {
                    canceller.cancel();
                    if let Some(transmit) = opt {
                        let (res, _) = socket.write(transmit).await;
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
                (res, buf) = socket.cancelable_read(Vec::with_capacity(1500), canceller.handle()) => {
                    match res {
                        Ok(n) => {
                            if n == 0 {
                                pipeline.read_eof();
                                break;
                            }

                            trace!("socket read {} bytes", n);
                            pipeline.read(BytesMut::from(&buf[..n]));
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
    }

    /// Gracefully stop the server
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
