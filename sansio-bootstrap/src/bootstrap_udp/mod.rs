use super::*;
use log::error;
use sansio_transport::{
    AsyncUdpSocket, BATCH_SIZE, Capabilities, RecvMeta, Transmit, TransportProtocol, UdpSocket,
};
use std::mem::MaybeUninit;

pub(crate) mod bootstrap_udp_client;
pub(crate) mod bootstrap_udp_server;

struct BootstrapUdp<W> {
    boostrap: Bootstrap<W>,

    socket: Option<UdpSocket>,
}

impl<W: 'static> Default for BootstrapUdp<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapUdp<W> {
    fn new() -> Self {
        Self {
            boostrap: Bootstrap::new(),

            socket: None,
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

    async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let socket = UdpSocket::bind(addr).await?;
        let local_addr = socket.local_addr()?;
        self.socket = Some(socket);
        Ok(local_addr)
    }

    async fn connect(
        &mut self,
        peer_addr: Option<SocketAddr>,
    ) -> Result<Rc<dyn RcOutboundPipeline<W>>, Error> {
        let socket = self.socket.take().unwrap();

        let pipeline_factory_fn = Rc::clone(self.boostrap.pipeline_factory_fn.as_ref().unwrap());
        let pipeline = (pipeline_factory_fn)(socket.local_addr()?, peer_addr);
        let pipeline_with_notify = PipelineWithNotify::new(Rc::clone(&pipeline));
        let pipeline_wr = pipeline;

        let (close_tx, close_rx) = broadcast::channel::<()>(1);
        {
            let mut tx = self.boostrap.close_tx.borrow_mut();
            *tx = Some(close_tx);
        }

        let wait_group = AsyncWaitGroup::new();

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
        socket: UdpSocket,
        max_payload_size: usize,
        pipeline_with_notify: PipelineWithNotify<TaggedBytesMut, W>,
        mut close_rx: broadcast::Receiver<()>,
    ) -> Result<(), Error> {
        let local_addr = socket.local_addr()?;

        let capabilities = Capabilities::new();
        let buf = vec![0u8; max_payload_size * capabilities.gro_segments() * BATCH_SIZE];
        let buf_len = buf.len();
        let mut recv_buf: Box<[u8]> = buf.into();
        let mut metas = [RecvMeta::default(); BATCH_SIZE];
        let mut iovs = MaybeUninit::<[std::io::IoSliceMut<'_>; BATCH_SIZE]>::uninit();
        recv_buf
            .chunks_mut(buf_len / BATCH_SIZE)
            .enumerate()
            .for_each(|(i, buf)| unsafe {
                iovs.as_mut_ptr()
                    .cast::<std::io::IoSliceMut<'_>>()
                    .add(i)
                    .write(std::io::IoSliceMut::<'_>::new(buf));
            });
        let mut iovs = unsafe { iovs.assume_init() };

        // Extract pipeline and write_notify from the wrapper
        let pipeline = pipeline_with_notify.pipeline();
        let write_notify = &pipeline_with_notify.write_notify;

        pipeline.transport_active();
        loop {
            // prioritize socket.write than socket.read
            while let Some(msg) = pipeline.poll_write() {
                let transmit = Transmit {
                    destination: msg.transport.peer_addr,
                    ecn: msg.transport.ecn,
                    contents: msg.message.to_vec(),
                    segment_size: None,
                    src_ip: Some(msg.transport.local_addr.ip()),
                };
                match socket.send(&capabilities, &[transmit]).await {
                    Ok(_) => {
                        trace!("socket write {} bytes", msg.message.len());
                    }
                    Err(err) => {
                        warn!("socket write error {}", err);
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
                    trace!("pipeline socket exit loop");
                    break;
                }
                _ = write_notify.notified() => {
                    // Wake up to write pending transmits
                    trace!("woken up by write notification");
                }
                _ = timer.as_mut() => {
                    pipeline.handle_timeout(Instant::now());
                }
                res = socket.recv(&mut iovs, &mut metas) => {
                    match res {
                        Ok(n) => {
                            if n == 0 {
                                pipeline.handle_eof();
                                break;
                            }

                            for (meta, buf) in metas.iter().zip(iovs.iter()).take(n) {
                                let message: BytesMut = buf[0..meta.len].into();
                                if !message.is_empty() {
                                    trace!("socket read {} bytes", message.len());
                                    pipeline.handle_read(TaggedBytesMut {
                                        now: Instant::now(),
                                        transport: TransportContext {
                                            local_addr,
                                            peer_addr: meta.addr,
                                            ecn: meta.ecn,
                                            transport_protocol: TransportProtocol::UDP,
                                        },
                                        message,
                                    });
                                }
                            }
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

        trace!("udp connection is gracefully down",);

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
