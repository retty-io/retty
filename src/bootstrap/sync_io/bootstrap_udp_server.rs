use bytes::BytesMut;
use log::{trace, warn};
use std::{
    io::{Error, ErrorKind},
    net::{ToSocketAddrs, UdpSocket},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::bootstrap::{PipelineFactoryFn, MIN_DURATION};
use crate::transport::{TaggedBytesMut, TransportContext};

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers.
pub struct BootstrapUdpServer<W> {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Arc<Mutex<Option<Sender<()>>>>,
    done_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl<W: Send + Sync + 'static> Default for BootstrapUdpServer<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Send + Sync + 'static> BootstrapUdpServer<W> {
    /// Creates a new BootstrapUdpServer
    pub fn new() -> Self {
        Self {
            pipeline_factory_fn: None,
            close_tx: Arc::new(Mutex::new(None)),
            done_rx: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapUdpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// Binds local address and port
    pub fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<(), Error> {
        let socket = Arc::new(UdpSocket::bind(addr)?);
        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());

        let (socket_rd, socket_wr) = (Arc::clone(&socket), socket);
        let (sender, receiver) = channel();
        let pipeline = (pipeline_factory_fn)(sender);

        let local_addr = socket_rd.local_addr()?;

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
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active();
            while close_rx.try_recv().is_err() {
                if let Ok(transmit) = receiver.try_recv() {
                    if let Some(peer_addr) = transmit.transport.peer_addr {
                        match socket_wr.send_to(&transmit.message, peer_addr) {
                            Ok(n) => {
                                trace!("socket write {} bytes", n);
                                continue;
                            }
                            Err(err) => {
                                warn!("socket write error {}", err);
                                break;
                            }
                        }
                    } else {
                        trace!("socket write error due to none peer_addr");
                        continue;
                    }
                }

                if let Some(evt) = pipeline.poll_event() {
                    pipeline.handle_event(evt);
                    continue;
                }

                // Default timeout in case no activity in order to gracefully shutdown
                let mut eto = Instant::now() + Duration::from_secs(MIN_DURATION);
                pipeline.poll_timeout(&mut eto);

                let timeout = eto
                    .checked_duration_since(Instant::now())
                    .unwrap_or(Duration::from_secs(0));

                if timeout.is_zero() {
                    pipeline.handle_timeout(Instant::now());
                    continue;
                }

                socket_rd.set_read_timeout(Some(timeout))?;
                match socket_rd.recv_from(&mut buf) {
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
                                peer_addr: Some(peer_addr),
                                ecn: None,
                            },
                            message: BytesMut::from(&buf[..n]),
                        });
                    }
                    Err(err) => match err.kind() {
                        // Expected error for set_read_timeout(). One for windows, one for the rest.
                        ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                            pipeline.handle_timeout(Instant::now());
                            continue;
                        }
                        _ => {
                            warn!("socket read error {}", err);
                            break;
                        }
                    },
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
                let _ = done_rx.recv();
            }
        }
    }
}
