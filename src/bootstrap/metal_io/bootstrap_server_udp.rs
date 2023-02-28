use bytes::BytesMut;
use log::{trace, warn};
use mio::net::UdpSocket;
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio_extras::{
    channel::{channel, Receiver, Sender},
    timer::Builder,
};
use std::{
    io::Error,
    net::ToSocketAddrs,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION_IN_SECS};
use crate::channel::InboundPipeline;
use crate::transport::{TaggedBytesMut, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers.
pub struct BootstrapServerUdp<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> Default for BootstrapServerUdp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Send + Sync + 'static> BootstrapServerUdp<W> {
    /// Creates a new BootstrapServerUdp
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapServerUdp::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let socket = Arc::new(super::each_addr(addr, UdpSocket::bind)?);
        let (socket_rd, socket_wr) = (Arc::clone(&socket), socket);
        let local_addr = socket_rd.local_addr()?;

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let (sender, receiver) = channel();
        let pipeline = (pipeline_factory_fn)(sender);

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
            poll.register(
                &socket_rd,
                SOCKET_RD_TOKEN,
                Ready::readable(),
                PollOpt::edge(),
            )?;
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
                                if let Some(peer_addr) = transmit.transport.peer_addr {
                                    match socket_wr.send_to(&transmit.message, &peer_addr) {
                                        Ok(n) => {
                                            trace!("socket write {} bytes", n);
                                        }
                                        Err(err) => {
                                            warn!("socket write error {}", err);
                                            break 'outer;
                                        }
                                    }
                                } else {
                                    trace!("socket write error due to none peer_addr");
                                }
                            }
                        }
                        SOCKET_RD_TOKEN => match socket_rd.recv_from(&mut buf) {
                            Ok((n, peer_addr)) => {
                                if n == 0 {
                                    pipeline.read_eof();
                                    break 'outer;
                                }

                                trace!("socket read {} bytes", n);
                                pipeline.read(TaggedBytesMut {
                                    now: Instant::now(),
                                    transport: TransportContext {
                                        local_addr,
                                        peer_addr: Some(peer_addr),
                                        ecn: None,
                                    },
                                    message: BytesMut::from(&buf[..n]),
                                });
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

            trace!("pipeline socket exit loop");
            let _ = done_tx.send(());

            Ok::<(), Error>(())
        });

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
            let mut done_rx = self.done_rx.lock().unwrap();
            if let Some(done_rx) = done_rx.take() {
                let _ = done_rx.try_recv(); //TODO: Loop until recv?
            }
        }
    }
}
