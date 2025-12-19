use super::*;
use log::error;
use sansio_transport::TransportProtocol;
use tokio::net::{TcpListener, TcpStream};

pub(crate) mod bootstrap_tcp_client;
pub(crate) mod bootstrap_tcp_server;

struct BootstrapTcp<W> {
    boostrap: Bootstrap<W>,
}

impl<W: 'static> Default for BootstrapTcp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapTcp<W> {
    fn new() -> Self {
        Self {
            boostrap: Bootstrap::new(),
        }
    }

    fn max_payload_size(&mut self, max_payload_size: usize) -> &mut Self {
        self.boostrap.max_payload_size(max_payload_size);
        self
    }

    fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>) -> &mut Self {
        self.boostrap.pipeline(pipeline_factory_fn);
        self
    }

    async fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<SocketAddr, Error> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        let pipeline_factory_fn = Rc::clone(self.boostrap.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, mut close_rx) = broadcast::channel::<()>(1);
        {
            let mut tx = self.boostrap.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let wait_group = AsyncWaitGroup::new();

        let max_payload_size = self.boostrap.max_payload_size;

        let worker = wait_group.add(1);
        spawn_local(async move {
            loop {
                tokio::select! {
                    biased;

                    _ = close_rx.recv() => {
                        trace!("listener exit loop");
                        break;
                    }
                    res = listener.accept() => {
                        match res {
                            Ok((socket, peer_addr)) => {
                                trace!("tcp connection from {}", peer_addr);
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let pipeline = (pipeline_factory_fn)(local_addr, Some(peer_addr));
                                let pipeline_with_notify = PipelineWithNotify::new(pipeline);
                                let child_close_rx = close_rx.resubscribe();
                                let child_worker = worker.add(1);
                                spawn_local(async move {
                                    if let Err(err) = Self::process_pipeline(socket,
                                                                   max_payload_size,
                                                                   pipeline_with_notify,
                                                                   child_close_rx).await {
                                        error!("process_pipeline got error: {}", err);
                                    }
                                    child_worker.done();
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

            worker.done();
        })
        .detach();

        {
            let mut wg = self.boostrap.wg.borrow_mut();
            *wg = Some(wait_group);
        }

        Ok(local_addr)
    }

    async fn connect<A: ToSocketAddrs>(
        &self,
        addr: A,
    ) -> Result<Rc<dyn RcOutboundPipeline<W>>, Error> {
        let socket = TcpStream::connect(addr).await?;
        let pipeline_factory_fn = Rc::clone(self.boostrap.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, close_rx) = broadcast::channel::<()>(1);
        {
            let mut tx = self.boostrap.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let wait_group = AsyncWaitGroup::new();

        let pipeline = (pipeline_factory_fn)(socket.local_addr()?, socket.peer_addr().ok());
        let pipeline_with_notify = PipelineWithNotify::new(Rc::clone(&pipeline));
        let pipeline_wr = pipeline;
        let max_payload_size = self.boostrap.max_payload_size;

        let worker = wait_group.add(1);
        spawn_local(async move {
            if let Err(err) =
                Self::process_pipeline(socket, max_payload_size, pipeline_with_notify, close_rx)
                    .await
            {
                error!("process_pipeline got error: {}", err);
            }
            worker.done();
        })
        .detach();

        {
            let mut wg = self.boostrap.wg.borrow_mut();
            *wg = Some(wait_group);
        }

        Ok(pipeline_wr)
    }

    async fn process_pipeline(
        mut stream: TcpStream,
        max_payload_size: usize,
        pipeline_with_notify: PipelineWithNotify<TaggedBytesMut, W>,
        mut close_rx: broadcast::Receiver<()>,
    ) -> Result<(), Error> {
        let local_addr = stream.local_addr()?;
        let peer_addr = stream.peer_addr()?;

        let mut buf = vec![0u8; max_payload_size];

        // Extract pipeline and write_notify from the wrapper
        let pipeline = pipeline_with_notify.pipeline();
        let write_notify = &pipeline_with_notify.write_notify;

        pipeline.transport_active();
        loop {
            // prioritize stream.write than stream.read
            while let Some(transmit) = pipeline.poll_write() {
                match stream.write(&transmit.message).await {
                    Ok(n) => {
                        trace!("stream write {} bytes", n);
                    }
                    Err(err) => {
                        warn!("stream write error {}", err);
                        break;
                    }
                }
            }

            let mut eto = Instant::now() + DEFAULT_TIMEOUT_DURATION;
            pipeline.poll_timeout(&mut eto);

            let delay_from_now = eto
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::from_secs(0));
            if delay_from_now.is_zero() {
                pipeline.handle_timeout(Instant::now());
                continue;
            }

            let timer = tokio::time::sleep(delay_from_now);
            tokio::pin!(timer);

            tokio::select! {
                biased;

                _ = close_rx.recv() => {
                    trace!("pipeline stream exit loop");
                    break;
                }
                _ = write_notify.notified() => {
                    // Wake up to write pending transmits
                    trace!("woken up by write notification");
                }
                _ = timer.as_mut() => {
                    pipeline.handle_timeout(Instant::now());
                }
                res = stream.read(&mut buf) => {
                    match res {
                        Ok(n) => {
                            if n == 0 {
                                pipeline.handle_eof();
                                break;
                            }

                            trace!("stream read {} bytes", n);
                            pipeline.handle_read(TaggedBytesMut {
                                    now: Instant::now(),
                                    transport: TransportContext {
                                        local_addr,
                                        peer_addr,
                                        ecn: None,
                                        transport_protocol: TransportProtocol::TCP,
                                    },
                                    message: BytesMut::from(&buf[..n]),
                                });
                        }
                        Err(err) => {
                            warn!("stream read error {}", err);
                            break;
                        }
                    }
                }
            }
        }
        pipeline.transport_inactive();

        trace!(
            "tcp connection on {} is gracefully down",
            stream.peer_addr()?
        );

        Ok(())
    }

    async fn stop(&self) {
        self.boostrap.stop().await
    }

    async fn wait_for_stop(&self) {
        self.boostrap.wait_for_stop().await
    }

    async fn graceful_stop(&self) {
        self.boostrap.graceful_stop().await
    }
}
