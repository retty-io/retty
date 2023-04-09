#[cfg(test)]
mod tests {
    use local_sync::mpsc::{unbounded::channel, unbounded::Tx as LocalSender};
    use log::trace;
    use std::cell::RefCell;
    use std::net::SocketAddr;
    use std::rc::Rc;
    use std::str::FromStr;
    use std::time::Instant;

    #[cfg(not(target_os = "linux"))]
    use retty::bootstrap::{BootstrapUdpClient, BootstrapUdpServer};

    use retty::bootstrap::{BootstrapTcpClient, BootstrapTcpServer};
    use retty::channel::{
        Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
    };
    use retty::codec::{
        byte_to_message_decoder::{
            LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType,
        },
        string_codec::{TaggedString, TaggedStringCodec},
    };
    use retty::executor::{spawn_local, try_yield_local, LocalExecutorBuilder};
    use retty::transport::{AsyncTransport, AsyncTransportWrite, TaggedBytesMut, TransportContext};

    ////////////////////////////////////////////////////////////////////////////////////////////////////

    struct EchoDecoder {
        is_server: bool,
        tx: Rc<RefCell<Option<LocalSender<()>>>>,
        count: Rc<RefCell<usize>>,
    }
    struct EchoEncoder;
    struct EchoHandler {
        decoder: EchoDecoder,
        encoder: EchoEncoder,
    }

    impl EchoHandler {
        fn new(
            is_server: bool,
            tx: Rc<RefCell<Option<LocalSender<()>>>>,
            count: Rc<RefCell<usize>>,
        ) -> Self {
            EchoHandler {
                decoder: EchoDecoder {
                    is_server,
                    tx,
                    count,
                },
                encoder: EchoEncoder,
            }
        }
    }

    impl InboundHandler for EchoDecoder {
        type Rin = TaggedString;
        type Rout = Self::Rin;

        fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
            {
                let mut count = self.count.borrow_mut();
                trace!(
                    "is_server = {}, count = {} msg = {}",
                    self.is_server,
                    *count,
                    msg.message
                );
                *count += 1;
            }

            if self.is_server {
                ctx.fire_write(TaggedString {
                    now: Instant::now(),
                    transport: msg.transport,
                    message: format!("{}\r\n", msg.message),
                });
            }

            if msg.message == "bye" {
                let mut tx = self.tx.borrow_mut();
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
            }
        }
        fn poll_timeout(
            &mut self,
            _ctx: &InboundContext<Self::Rin, Self::Rout>,
            _eto: &mut Instant,
        ) {
            //last handler, no need to fire_poll_timeout
        }
    }

    impl OutboundHandler for EchoEncoder {
        type Win = TaggedString;
        type Wout = Self::Win;

        fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
            ctx.fire_write(msg);
        }
    }

    impl Handler for EchoHandler {
        type Rin = TaggedString;
        type Rout = Self::Rin;
        type Win = TaggedString;
        type Wout = Self::Win;

        fn name(&self) -> &str {
            "EchoHandler"
        }

        fn split(
            self,
        ) -> (
            Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
            Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
        ) {
            (Box::new(self.decoder), Box::new(self.encoder))
        }
    }

    //TODO: test_echo_udp passed in my local linux(5.15.77-amd64-desktop), but always hang in
    // github actions or codespaces linux version (5.4.0-1104-azure).
    // echo/chat_server_udp and client_udp work fine for both linux version.
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_echo_udp() {
        LocalExecutorBuilder::default().run(async {
            const ITER: usize = 1024;

            let (tx, mut rx) = channel();

            let server_count = Rc::new(RefCell::new(0));
            let server_count_clone = server_count.clone();
            let (server_done_tx, mut server_done_rx) = channel();
            let server_done_tx = Rc::new(RefCell::new(Some(server_done_tx)));

            let mut server = BootstrapUdpServer::new();
            server.pipeline(Box::new(
                move |writer: AsyncTransportWrite<TaggedBytesMut>| {
                    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                    let async_transport_handler = AsyncTransport::new(writer);
                    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                    ));
                    let string_codec_handler = TaggedStringCodec::new();
                    let echo_handler = EchoHandler::new(
                        true,
                        Rc::clone(&server_done_tx),
                        Rc::clone(&server_count_clone),
                    );

                    pipeline.add_back(async_transport_handler);
                    pipeline.add_back(line_based_frame_decoder_handler);
                    pipeline.add_back(string_codec_handler);
                    pipeline.add_back(echo_handler);
                    pipeline.finalize()
                },
            ));

            let server_addr = server.bind("127.0.0.1:0").await.unwrap();

            spawn_local(async move {
                let client_count = Rc::new(RefCell::new(0));
                let client_count_clone = client_count.clone();
                let (client_done_tx, mut client_done_rx) = channel();
                let client_done_tx = Rc::new(RefCell::new(Some(client_done_tx)));

                let mut client = BootstrapUdpClient::new();
                client.pipeline(Box::new(
                    move |writer: AsyncTransportWrite<TaggedBytesMut>| {
                        let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                        let async_transport_handler = AsyncTransport::new(writer);
                        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(
                            Box::new(LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH)),
                        );
                        let string_codec_handler = TaggedStringCodec::new();
                        let echo_handler = EchoHandler::new(
                            false,
                            Rc::clone(&client_done_tx),
                            Rc::clone(&client_count_clone),
                        );

                        pipeline.add_back(async_transport_handler);
                        pipeline.add_back(line_based_frame_decoder_handler);
                        pipeline.add_back(string_codec_handler);
                        pipeline.add_back(echo_handler);
                        pipeline.finalize()
                    },
                ));

                let client_addr = client.bind("127.0.0.1:0").await.unwrap();
                let pipeline = client.connect(server_addr).await.unwrap();

                for i in 0..ITER {
                    // write
                    pipeline.write(TaggedString {
                        now: Instant::now(),
                        transport: TransportContext {
                            local_addr: client_addr,
                            peer_addr: Some(server_addr),
                            ecn: None,
                        },
                        message: format!("{}\r\n", i),
                    });
                }
                pipeline.write(TaggedString {
                    now: Instant::now(),
                    transport: TransportContext {
                        local_addr: client_addr,
                        peer_addr: Some(server_addr),
                        ecn: None,
                    },
                    message: format!("bye\r\n"),
                });

                assert!(client_done_rx.recv().await.is_some());

                assert!(tx.send(client_count).is_ok());

                client.stop().await;
            })
            .detach();

            let client_count = rx.recv().await.unwrap();
            assert!(server_done_rx.recv().await.is_some());

            let (client_count, server_count) = (client_count.borrow(), server_count.borrow());
            assert_eq!(*client_count, *server_count);
            assert_eq!(ITER + 1, *client_count);

            server.stop().await;
        });
    }

    #[test]
    fn test_echo_tcp() {
        LocalExecutorBuilder::default().run(async {
            const ITER: usize = 1024;

            let (tx, mut rx) = channel();

            let server_count = Rc::new(RefCell::new(0));
            let server_count_clone = server_count.clone();
            let (server_done_tx, mut server_done_rx) = channel();
            let server_done_tx = Rc::new(RefCell::new(Some(server_done_tx)));

            let mut server = BootstrapTcpServer::new();
            server.pipeline(Box::new(
                move |writer: AsyncTransportWrite<TaggedBytesMut>| {
                    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                    let async_transport_handler = AsyncTransport::new(writer);
                    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                    ));
                    let string_codec_handler = TaggedStringCodec::new();
                    let echo_handler = EchoHandler::new(
                        true,
                        Rc::clone(&server_done_tx),
                        Rc::clone(&server_count_clone),
                    );

                    pipeline.add_back(async_transport_handler);
                    pipeline.add_back(line_based_frame_decoder_handler);
                    pipeline.add_back(string_codec_handler);
                    pipeline.add_back(echo_handler);
                    pipeline.finalize()
                },
            ));

            let server_addr = server.bind("127.0.0.1:0").await.unwrap();

            spawn_local(async move {
                let client_count = Rc::new(RefCell::new(0));
                let client_count_clone = client_count.clone();
                let (client_done_tx, mut client_done_rx) = channel();
                let client_done_tx = Rc::new(RefCell::new(Some(client_done_tx)));

                let mut client = BootstrapTcpClient::new();
                client.pipeline(Box::new(
                    move |writer: AsyncTransportWrite<TaggedBytesMut>| {
                        let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                        let async_transport_handler = AsyncTransport::new(writer);
                        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(
                            Box::new(LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH)),
                        );
                        let string_codec_handler = TaggedStringCodec::new();
                        let echo_handler = EchoHandler::new(
                            false,
                            Rc::clone(&client_done_tx),
                            Rc::clone(&client_count_clone),
                        );

                        pipeline.add_back(async_transport_handler);
                        pipeline.add_back(line_based_frame_decoder_handler);
                        pipeline.add_back(string_codec_handler);
                        pipeline.add_back(echo_handler);
                        pipeline.finalize()
                    },
                ));

                let client_addr = SocketAddr::from_str("127.0.0.1:0").unwrap();

                let pipeline = client.connect(server_addr).await.unwrap();

                for i in 0..ITER {
                    // write
                    pipeline.write(TaggedString {
                        now: Instant::now(),
                        transport: TransportContext {
                            local_addr: client_addr,
                            peer_addr: None,
                            ecn: None,
                        },
                        message: format!("{}\r\n", i),
                    });
                    try_yield_local();
                }
                pipeline.write(TaggedString {
                    now: Instant::now(),
                    transport: TransportContext {
                        local_addr: client_addr,
                        peer_addr: None,
                        ecn: None,
                    },
                    message: format!("bye\r\n"),
                });
                try_yield_local();

                assert!(client_done_rx.recv().await.is_some());

                assert!(tx.send(client_count).is_ok());

                client.stop().await;
            })
            .detach();

            let client_count = rx.recv().await.unwrap();
            assert!(server_done_rx.recv().await.is_some());

            let (client_count, server_count) = (client_count.borrow(), server_count.borrow());
            assert_eq!(*client_count, *server_count);
            assert_eq!(ITER + 1, *client_count);

            server.stop().await;
        });
    }
}
