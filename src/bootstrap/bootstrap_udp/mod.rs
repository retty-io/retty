use super::*;
use crate::transport::Protocol;
use async_transport::{AsyncUdpSocket, Capabilities, RecvMeta, Transmit, UdpSocket, BATCH_SIZE};
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

    async fn bind<A: AsyncToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let socket = UdpSocket::bind(addr).await?;
        let local_addr = socket.local_addr()?;
        self.socket = Some(socket);
        Ok(local_addr)
    }

    async fn connect(
        &mut self,
        _peer_addr: Option<SocketAddr>,
    ) -> Result<Rc<dyn OutboundPipeline<TaggedBytesMut, W>>, Error> {
        let socket = self.socket.take().unwrap();
        let local_addr = socket.local_addr()?;

        let pipeline_factory_fn = Rc::clone(self.boostrap.pipeline_factory_fn.as_ref().unwrap());
        let pipeline = (pipeline_factory_fn)();
        let pipeline_wr = Rc::clone(&pipeline);

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

        let max_payload_size = self.boostrap.max_payload_size;

        spawn_local(async move {
            let _w = worker;

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

            pipeline.transport_active();
            loop {
                // prioritize socket.write than socket.read
                while let Some(msg) = pipeline.poll_transmit() {
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
                    res = socket.recv(&mut iovs, &mut metas) => {
                        match res {
                            Ok(n) => {
                                if n == 0 {
                                    pipeline.handle_read_eof();
                                    break;
                                }

                                for (meta, buf) in metas.iter().zip(iovs.iter()).take(n) {
                                    let message: BytesMut = buf[0..meta.len].into();
                                    if !message.is_empty() {
                                        trace!("socket read {} bytes", message.len());
                                        pipeline
                                            .read(TaggedBytesMut {
                                                now: Instant::now(),
                                                transport: TransportContext {
                                                    local_addr,
                                                    peer_addr: meta.addr,
                                                    ecn: meta.ecn,
                                                    protocol: Protocol::UDP,
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
        })
        .detach();

        Ok(pipeline_wr)
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
