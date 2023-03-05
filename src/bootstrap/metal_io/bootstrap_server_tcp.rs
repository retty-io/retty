use bytes::BytesMut;
use crossbeam::sync::WaitGroup;
use log::{trace, warn};
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, PollOpt, Ready, Token};
use retty_io::timer::Builder;
use std::{
    io::{Error, ErrorKind, Read, Write},
    net::ToSocketAddrs,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::InboundPipeline;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapServerTcp<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<BytesMut, W>>>,
    close_tx: Arc<Mutex<Option<retty_io::broadcast::Sender<()>>>>,
    wg: Arc<Mutex<Option<WaitGroup>>>,
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
            wg: Arc::new(Mutex::new(None)),
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

        let (close_tx, close_rx) = retty_io::broadcast::channel();
        {
            let mut tx = self.close_tx.lock().unwrap();
            *tx = Some(close_tx);
        }

        let worker = {
            let workgroup = WaitGroup::new();
            let worker = workgroup.clone();
            {
                let mut wg = self.wg.lock().unwrap();
                *wg = Some(workgroup);
            }
            worker
        };

        thread::spawn(move || {
            let _w = worker;
            let child_wg = WaitGroup::new();

            const LISTENER_TOKEN: Token = Token(0);
            const CLOSE_RX_TOKEN: Token = Token(1);

            let poll = Poll::new()?;
            poll.register(
                &listener,
                LISTENER_TOKEN,
                Ready::readable(),
                PollOpt::edge(),
            )?;
            poll.register(
                &close_rx,
                CLOSE_RX_TOKEN,
                Ready::readable(),
                PollOpt::edge(),
            )?;

            let mut events = Events::with_capacity(128);

            'outer: loop {
                poll.poll(&mut events, None)?;
                for event in events.iter() {
                    match event.token() {
                        CLOSE_RX_TOKEN => {
                            let _ = close_rx.try_recv();
                            trace!("listener exit loop");
                            break 'outer;
                        }
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
                                    warn!("listener accept error {}", e);
                                    break 'outer;
                                }
                            };

                            trace!("Accepted connection from: {}", peer);
                            let child_pipeline_factory_fn = Arc::clone(&pipeline_factory_fn);
                            let child_worker = child_wg.clone();
                            let child_close_rx = close_rx.clone();
                            thread::spawn(move || {
                                let _ = Self::process_pipeline(
                                    socket,
                                    child_pipeline_factory_fn,
                                    child_close_rx,
                                    child_worker,
                                );
                            });
                        },
                        _ => unreachable!(),
                    }
                }
            }
            child_wg.wait();

            Ok::<(), Error>(())
        });

        Ok(())
    }

    fn process_pipeline(
        mut socket: TcpStream,
        pipeline_factory_fn: Arc<PipelineFactoryFn<BytesMut, W>>,
        close_rx: retty_io::broadcast::Receiver<()>,
        worker: WaitGroup,
    ) -> Result<(), Error> {
        let _w = worker;

        let (sender, receiver) = retty_io::channel::channel();
        let pipeline = (pipeline_factory_fn)(sender);

        const SOCKET_WT_TOKEN: Token = Token(0);
        const SOCKET_RD_TOKEN: Token = Token(1);
        const CLOSE_RX_TOKEN: Token = Token(2);
        const TIMEOUT_TOKEN: Token = Token(3);
        const OUTBOUND_EVENT_TOKEN: Token = Token(4);

        let mut timer = Builder::default().build::<()>();
        let poll = Poll::new()?;
        poll.register(
            &receiver,
            SOCKET_WT_TOKEN,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        poll.register(&socket, SOCKET_RD_TOKEN, Ready::readable(), PollOpt::edge())?;
        poll.register(
            &close_rx,
            CLOSE_RX_TOKEN,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        poll.register(&timer, TIMEOUT_TOKEN, Ready::readable(), PollOpt::edge())?;
        poll.register(
            &pipeline,
            OUTBOUND_EVENT_TOKEN,
            Ready::readable(),
            PollOpt::edge(),
        )?;

        let mut events = Events::with_capacity(128);

        let mut buf = vec![0u8; 8196];

        pipeline.transport_active();
        'outer: loop {
            let mut eto = Instant::now() + Duration::from_secs(MAX_DURATION_IN_SECS);
            pipeline.poll_timeout(&mut eto);
            let delay_from_now = eto
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::from_secs(0));
            if delay_from_now.is_zero() {
                pipeline.handle_timeout(Instant::now());
                continue;
            }

            let timeout = timer.set_timeout(delay_from_now, ());
            let _timeout = super::TimeoutGuard::new(&mut timer, timeout);

            poll.poll(&mut events, None)?;
            for event in events.iter() {
                match event.token() {
                    SOCKET_WT_TOKEN => {
                        if let Ok(transmit) = receiver.try_recv() {
                            match socket.write(&transmit) {
                                Ok(n) => {
                                    trace!("socket write {} bytes", n);
                                }
                                Err(err) => {
                                    warn!("socket write error {}", err);
                                    break 'outer;
                                }
                            }
                        }
                    }
                    SOCKET_RD_TOKEN => match socket.read(&mut buf) {
                        Ok(n) => {
                            if n == 0 {
                                pipeline.read_eof();
                                break 'outer;
                            }

                            trace!("socket read {} bytes", n);
                            pipeline.read(BytesMut::from(&buf[..n]));
                        }
                        Err(err) => {
                            warn!("socket read error {}", err);
                            break 'outer;
                        }
                    },
                    CLOSE_RX_TOKEN => {
                        let _ = close_rx.try_recv();
                        break 'outer;
                    }
                    TIMEOUT_TOKEN => {
                        pipeline.handle_timeout(Instant::now());
                    }
                    OUTBOUND_EVENT_TOKEN => {
                        if let Some(evt) = pipeline.poll_outbound_event() {
                            pipeline.handle_outbound_event(evt);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        pipeline.transport_inactive();

        Ok(())
    }

    /// Gracefully stop the server
    pub fn stop(&self) {
        {
            let mut close_tx = self.close_tx.lock().unwrap();
            if let Some(close_tx) = close_tx.take() {
                let _ = close_tx.send(());
            }
        }
        {
            let mut wg = self.wg.lock().unwrap();
            if let Some(wg) = wg.take() {
                wg.wait();
            }
        }
    }
}
