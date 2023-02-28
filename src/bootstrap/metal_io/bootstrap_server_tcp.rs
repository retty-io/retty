use bytes::BytesMut;
use log::{trace, warn};
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio_extras::{
    channel::{channel, Receiver, Sender},
    timer::Builder,
};
use std::collections::HashMap;
use std::{
    io::{Error, ErrorKind, Read, Write},
    net::ToSocketAddrs,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::{InboundPipeline, OutboundPipeline};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapServerTcp<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<BytesMut, W>>>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> Default for BootstrapServerTcp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Send + Sync + 'static> BootstrapServerTcp<W> {
    /// Creates a new BootstrapServerTcp
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapServerTcp::bind].
    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<BytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), Error> {
        let listener = super::each_addr(addr, TcpListener::bind)?;

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (close_tx, close_rx) = channel();
        {
            let mut tx = self.close_tx.lock().unwrap();
            *tx = Some(close_tx);
        }

        let (done_tx, done_rx) = channel();
        {
            let mut rx = self.done_rx.lock().unwrap();
            *rx = Some(done_rx);
        }

        thread::spawn(move || {
            const LISTENER_TOKEN: Token = Token(0);

            let poll = Poll::new()?;
            poll.register(
                &listener,
                LISTENER_TOKEN,
                Ready::readable(),
                PollOpt::edge(),
            )?;

            let mut events = Events::with_capacity(128);

            // Map of `Token` -> `TcpStream`.
            let mut connections = HashMap::new();
            // Unique token for each incoming connection.
            let mut unique_token = Token(LISTENER_TOKEN.0 + 1);

            'outer: loop {
                poll.poll(&mut events, None)?;
                for event in events.iter() {
                    match event.token() {
                        LISTENER_TOKEN => loop {
                            // Received an event for the TCP server socket, which
                            // indicates we can accept an connection.
                            let (socket, peer) = match listener.accept() {
                                Ok((socket, peer)) => (socket, peer),
                                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                    // If we get a `WouldBlock` error we know our
                                    // listener has no more incoming connections queued,
                                    // so we can return to polling and wait for some
                                    // more.
                                    break;
                                }
                                Err(e) => {
                                    // If it was any other kind of error, something went
                                    // wrong and we terminate with an error.
                                    return Err(e);
                                }
                            };

                            trace!("Accepted connection from: {}", peer);

                            let token = Self::next(&mut unique_token);
                            /*poll.registry().register(
                                &mut connection,
                                token,
                                Interest::READABLE.add(Interest::WRITABLE),
                            )?;*/

                            connections.insert(token, socket);
                        },
                        _token => {}
                    }
                }

                /*
                tokio::select! {
                    _ = close_rx.recv() => {
                        trace!("listener exit loop");
                        break;
                    }

                    res = listener.accept() => {
                        match res {
                            Ok((socket, _remote_addr)) => {
                                // A new task is spawned for each inbound socket. The socket is
                                // moved to the new task and processed there.
                                let child_pipeline_factory_fn = Arc::clone(&pipeline_factory_fn);

                                #[cfg(feature = "runtime-tokio")]
                                let child_close_rx = {
                                    let tx = close_tx.lock().await;
                                    if let Some(t) = &*tx {
                                        t.subscribe()
                                    } else {
                                        warn!("BootstrapServerTcp is closed");
                                        break
                                    }
                                };
                                #[cfg(feature = "runtime-async-std")]
                                let child_close_rx = {
                                    close_rx.clone()
                                };

                                let child_worker = child_wg.worker();
                                runtime.spawn(Box::pin(async move {
                                    BootstrapTcpServer::process_pipeline(socket, child_pipeline_factory_fn, child_close_rx, child_worker)
                                        .await;
                                }));
                            }
                            Err(err) => {
                                warn!("listener accept error {}", err);
                                break;
                            }
                        };
                    }
                }*/
            }

            Ok::<(), Error>(())
        });

        Ok(())
    }

    fn register_pipeline() {}

    /*
    fn process_pipeline(
        socket: TcpStream,
        pipeline_factory_fn: Arc<PipelineFactoryFn<BytesMut, W>>,
        #[allow(unused_mut)] mut close_rx: Receiver<()>,
        worker: Worker,
    ) {
        let _w = worker;
        let mut buf = vec![0u8; 8196];

        #[cfg(feature = "runtime-tokio")]
        let (mut socket_rd, mut socket_wr) = socket.into_split();
        #[cfg(feature = "runtime-async-std")]
        let (mut socket_rd, mut socket_wr) = (socket.clone(), socket);

        let (sender, mut receiver) = channel(8); //TODO: make it configurable?
        let pipeline = (pipeline_factory_fn)(sender).await;

        pipeline.transport_active().await;
        loop {
            if let Ok(transmit) = receiver.try_recv() {
                match socket_wr.write(&transmit, None).await {
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
                    break;
                }
                _ = timer.as_mut() => {
                    pipeline.handle_timeout(Instant::now()).await;
                }
                res = receiver.recv() => {
                    match res {
                        Ok(transmit) => {
                            match socket_wr.write(&transmit, None).await {
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
                        Err(err) => {
                            warn!("pipeline recv error {}", err);
                            break;
                        }
                    }
                }
                res = socket_rd.read(&mut buf) => {
                    match res {
                        Ok((n,_)) => {
                            if n == 0 {
                                pipeline.read_eof().await;
                                break;
                            }

                            trace!("socket read {} bytes", n);
                            pipeline.read(BytesMut::from(&buf[..n])).await;
                        }
                        Err(err) => {
                            warn!("socket read error {}", err);
                            break;
                        }
                    };
                }
            }
        }
        pipeline.transport_inactive();
    }*/

    /// Gracefully stop the server
    pub fn stop(&self) {
        {
            let mut close_tx = self.close_tx.lock().unwrap();
            if let Some(close_tx) = close_tx.take() {
                let _ = close_tx.send(());
            }
        }
        {
            let mut done_rx = self.done_rx.lock().unwrap();
            if let Some(done_rx) = done_rx.take() {
                let _ = done_rx.try_recv(); //TODO: Loop until recv?
            }
        }
    }

    fn next(current: &mut Token) -> Token {
        let next = current.0;
        current.0 += 5;
        /*
           const SOCKET_WT_TOKEN: Token = Token(0);
           const SOCKET_RD_TOKEN: Token = Token(1);
           const CLOSE_RX_TOKEN: Token = Token(2);
           const TIMEOUT_TOKEN: Token = Token(3);
           const OUTBOUND_EVENT_TOKEN: Token = Token(4);
        */
        Token(next)
    }
}
