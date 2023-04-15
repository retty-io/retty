use super::*;
use smol::{
    net::{AsyncToSocketAddrs, TcpListener, TcpStream},
    Timer,
};

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

    fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>) -> &mut Self {
        self.boostrap.pipeline(pipeline_factory_fn);
        self
    }

    async fn bind<A: AsyncToSocketAddrs>(&self, addr: A) -> Result<SocketAddr, Error> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        let pipeline_factory_fn = Rc::clone(self.boostrap.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, mut close_rx) = async_broadcast::broadcast(1);
        {
            let mut tx = self.boostrap.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let worker = {
            let workgroup = WaitGroup::new();
            let worker = workgroup.worker();
            {
                let mut wg = self.boostrap.wg.borrow_mut();
                *wg = Some(workgroup);
            }
            worker
        };

        spawn_local(async move {
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
                            Ok((socket, peer_addr)) => {
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let (sender, receiver) = channel();
                                let pipeline_rd = (pipeline_factory_fn)(AsyncTransportWrite {
                                    sender,
                                    transport: TransportContext {
                                        local_addr,
                                        peer_addr: Some(peer_addr),
                                        ecn: None,
                                    },
                                });
                                let child_close_rx = close_rx.clone();
                                let child_worker = child_wg.worker();
                                spawn_local(async move {
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

    async fn connect<A: AsyncToSocketAddrs>(
        &self,
        addr: A,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        let socket = TcpStream::connect(addr).await?;
        let local_addr = socket.local_addr()?;
        let peer_addr = socket.peer_addr()?;
        let pipeline_factory_fn = Rc::clone(self.boostrap.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, close_rx) = async_broadcast::broadcast(1);
        {
            let mut tx = self.boostrap.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let worker = {
            let workgroup = WaitGroup::new();
            let worker = workgroup.worker();
            {
                let mut wg = self.boostrap.wg.borrow_mut();
                *wg = Some(workgroup);
            }
            worker
        };

        let (sender, receiver) = channel();
        let pipeline_rd = (pipeline_factory_fn)(AsyncTransportWrite {
            sender,
            transport: TransportContext {
                local_addr,
                peer_addr: Some(peer_addr),
                ecn: None,
            },
        });
        let pipeline_wr = Rc::clone(&pipeline_rd);
        spawn_local(async move {
            let _ = Self::process_pipeline(socket, pipeline_rd, receiver, close_rx, worker).await;
        })
        .detach();

        Ok(pipeline_wr)
    }

    async fn process_pipeline(
        mut socket: TcpStream,
        pipeline: Rc<dyn InboundPipeline<TaggedBytesMut>>,
        mut receiver: LocalReceiver<TaggedBytesMut>,
        mut close_rx: async_broadcast::Receiver<()>,
        worker: Worker,
    ) -> Result<(), Error> {
        let _w = worker;

        let local_addr = socket.local_addr()?;

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
        self.boostrap.stop().await
    }

    async fn wait_for_stop(&self) {
        self.boostrap.wait_for_stop().await
    }

    async fn graceful_stop(&self) {
        self.boostrap.graceful_stop().await
    }
}
