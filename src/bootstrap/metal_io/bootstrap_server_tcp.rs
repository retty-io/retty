use bytes::BytesMut;
use log::{trace, warn};
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio_extras::{
    channel::{channel, Receiver, Sender},
    timer::{Builder, Timer},
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
use crate::channel::{InboundPipeline, Pipeline};

enum ConnectionToken {
    SocketWt,
    SocketRd,
    Timeout,
    OutboundEvent,
    Num,
}

impl From<usize> for ConnectionToken {
    fn from(val: usize) -> Self {
        match val {
            0 => Self::SocketWt,
            1 => Self::SocketRd,
            2 => Self::Timeout,
            3 => Self::OutboundEvent,
            _ => Self::Num,
        }
    }
}

struct Connection<W> {
    receiver: Receiver<BytesMut>,
    socket: TcpStream,
    pipeline: Arc<Pipeline<BytesMut, W>>,
}

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
            const LISTENER_TOKEN: Token = Token(usize::MAX);
            const CLOSE_RX_TOKEN: Token = Token(usize::MAX - 1);

            let mut timer = Builder::default().build::<()>();
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

            // Map of `usize` -> `Connection`.
            let mut connections: HashMap<usize, Connection<W>> = HashMap::new();
            // Unique token for each incoming connection.
            let mut unique_token = Token(0);

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
                                    warn!("listener accept error {}", e);
                                    break 'outer;
                                }
                            };

                            trace!("Accepted connection from: {}", peer);
                            Self::register_connection(
                                &pipeline_factory_fn,
                                &mut connections,
                                &mut unique_token,
                                &poll,
                                &timer,
                                socket,
                            )?;
                        },
                        CLOSE_RX_TOKEN => {
                            let _ = close_rx.try_recv();
                            //TODO: clean up connections?
                            break 'outer;
                        }
                        token => {
                            let key = token.0 % 4;
                            if let Some(connection) = connections.get_mut(&key) {
                                Self::process_connection(connection, token, &mut timer);
                            }
                        }
                    }
                }
            }

            trace!("listener exit loop");
            let _ = done_tx.send(());

            Ok::<(), Error>(())
        });

        Ok(())
    }

    fn register_connection(
        pipeline_factory_fn: &Arc<PipelineFactoryFn<BytesMut, W>>,
        connections: &mut HashMap<usize, Connection<W>>,
        token: &mut Token,
        poll: &Poll,
        timer: &Timer<()>,
        socket: TcpStream,
    ) -> Result<(), Error> {
        let (sender, receiver) = channel();
        let pipeline = (pipeline_factory_fn)(sender);

        #[allow(non_snake_case)]
        let SOCKET_WT_TOKEN: Token = Token(token.0 + ConnectionToken::SocketWt as usize);
        #[allow(non_snake_case)]
        let SOCKET_RD_TOKEN: Token = Token(token.0 + ConnectionToken::SocketRd as usize);
        #[allow(non_snake_case)]
        let TIMEOUT_TOKEN: Token = Token(token.0 + ConnectionToken::Timeout as usize);
        #[allow(non_snake_case)]
        let OUTBOUND_EVENT_TOKEN: Token = Token(token.0 + ConnectionToken::OutboundEvent as usize);

        poll.register(
            &receiver,
            SOCKET_WT_TOKEN,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        poll.register(&socket, SOCKET_RD_TOKEN, Ready::readable(), PollOpt::edge())?;
        poll.register(timer, TIMEOUT_TOKEN, Ready::readable(), PollOpt::edge())?;
        poll.register(
            &pipeline,
            OUTBOUND_EVENT_TOKEN,
            Ready::readable(),
            PollOpt::edge(),
        )?;

        connections.insert(
            token.0,
            Connection {
                receiver,
                socket,
                pipeline,
            },
        );
        token.0 += ConnectionToken::Num as usize;

        Ok(())
    }

    fn process_connection(connection: &mut Connection<W>, token: Token, timer: &mut Timer<()>) {
        let mut buf = vec![0u8; 8196];

        connection.pipeline.transport_active();
        'outer: loop {
            let mut eto = Instant::now() + Duration::from_secs(MAX_DURATION_IN_SECS);
            connection.pipeline.poll_timeout(&mut eto);
            let delay_from_now = eto
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::from_secs(0));
            if delay_from_now.is_zero() {
                connection.pipeline.handle_timeout(Instant::now());
                continue;
            }

            let timeout = timer.set_timeout(delay_from_now, ());
            let _timeout = super::TimeoutGuard::new(timer, timeout);

            match ConnectionToken::from(token.0 % ConnectionToken::Num as usize) {
                ConnectionToken::SocketWt => {
                    if let Ok(transmit) = connection.receiver.try_recv() {
                        match connection.socket.write(&transmit) {
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
                ConnectionToken::SocketRd => match connection.socket.read(&mut buf) {
                    Ok(n) => {
                        if n == 0 {
                            connection.pipeline.read_eof();
                            break 'outer;
                        }

                        trace!("socket read {} bytes", n);
                        connection.pipeline.read(BytesMut::from(&buf[..n]));
                    }
                    Err(err) => {
                        warn!("socket read error {}", err);
                        break 'outer;
                    }
                },
                ConnectionToken::Timeout => {
                    connection.pipeline.handle_timeout(Instant::now());
                }
                ConnectionToken::OutboundEvent => {
                    if let Some(evt) = connection.pipeline.poll_outbound_event() {
                        connection.pipeline.handle_outbound_event(evt);
                    }
                }
                _ => unreachable!(),
            }
        }
        connection.pipeline.transport_inactive();
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
            let mut done_rx = self.done_rx.lock().unwrap();
            if let Some(done_rx) = done_rx.take() {
                let _ = done_rx.try_recv(); //TODO: Loop until recv?
            }
        }
    }
}
