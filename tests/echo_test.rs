#[cfg(test)]
mod tests {
    use core_affinity::CoreId;
    use local_sync::mpsc::{unbounded::channel, unbounded::Tx as LocalSender};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::net::SocketAddr;
    use std::rc::Rc;
    use std::str::FromStr;
    use std::time::Instant;

    use retty::bootstrap::{
        BootstrapTcpClient, BootstrapTcpServer, BootstrapUdpClient, BootstrapUdpServer,
    };
    use retty::channel::{Context, Handler, Pipeline};
    use retty::codec::{
        byte_to_message_decoder::{
            LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType,
        },
        string_codec::TaggedStringCodec,
    };
    use retty::executor::{spawn_local, yield_local, LocalExecutorBuilder};
    use retty::transport::{
        EcnCodepoint, Protocol, TaggedBytesMut, TaggedString, TransportContext,
    };

    ////////////////////////////////////////////////////////////////////////////////////////////////////
    struct EchoHandler {
        is_server: bool,
        tx: Rc<RefCell<Option<LocalSender<()>>>>,
        count: Rc<RefCell<usize>>,
        check_ecn: bool,
        transmits: VecDeque<TaggedString>,
    }

    impl EchoHandler {
        fn new(
            is_server: bool,
            tx: Rc<RefCell<Option<LocalSender<()>>>>,
            count: Rc<RefCell<usize>>,
            check_ecn: bool,
        ) -> Self {
            EchoHandler {
                is_server,
                tx,
                count,
                check_ecn,
                transmits: VecDeque::new(),
            }
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

        fn handle_read(
            &mut self,
            _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
            msg: Self::Rin,
        ) {
            {
                let mut count = self.count.borrow_mut();
                println!(
                    "is_server = {}, count = {} msg = {} with ECN = {:?}",
                    self.is_server, *count, msg.message, msg.transport.ecn
                );
                *count += 1;
                if self.check_ecn {
                    assert_eq!(Some(EcnCodepoint::Ect1), msg.transport.ecn);
                }
            }

            if self.is_server {
                self.transmits.push_back(TaggedString {
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
            _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
            _eto: &mut Instant,
        ) {
            //last handler, no need to fire_poll_timeout
        }

        fn poll_write(
            &mut self,
            ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        ) -> Option<Self::Wout> {
            if let Some(msg) = ctx.fire_poll_write() {
                self.transmits.push_back(msg);
            }
            self.transmits.pop_front()
        }
    }

    #[test]
    fn test_echo_udp() {
        let handler = LocalExecutorBuilder::new()
            .name("ech_udp_thread")
            .core_id(CoreId { id: 0 })
            .spawn(|| async move {
                const ITER: usize = 100; //1024;

                let (tx, mut rx) = channel();

                let server_count = Rc::new(RefCell::new(0));
                let server_count_clone = server_count.clone();
                let (server_done_tx, mut server_done_rx) = channel();
                let server_done_tx = Rc::new(RefCell::new(Some(server_done_tx)));

                let mut server = BootstrapUdpServer::new();
                server.pipeline(Box::new(move || {
                    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                    ));
                    let string_codec_handler = TaggedStringCodec::new();
                    let echo_handler = EchoHandler::new(
                        true,
                        Rc::clone(&server_done_tx),
                        Rc::clone(&server_count_clone),
                        #[cfg(not(target_os = "windows"))]
                        true,
                        #[cfg(target_os = "windows")]
                        false,
                    );

                    pipeline.add_back(line_based_frame_decoder_handler);
                    pipeline.add_back(string_codec_handler);
                    pipeline.add_back(echo_handler);
                    pipeline.finalize()
                }));

                let server_addr = server.bind("127.0.0.1:0").await.unwrap();

                spawn_local(async move {
                    let client_count = Rc::new(RefCell::new(0));
                    let client_count_clone = client_count.clone();
                    let (client_done_tx, mut client_done_rx) = channel();
                    let client_done_tx = Rc::new(RefCell::new(Some(client_done_tx)));

                    let mut client = BootstrapUdpClient::new();
                    client.pipeline(Box::new(move || {
                        let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(
                            Box::new(LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH)),
                        );
                        let string_codec_handler = TaggedStringCodec::new();
                        let echo_handler = EchoHandler::new(
                            false,
                            Rc::clone(&client_done_tx),
                            Rc::clone(&client_count_clone),
                            #[cfg(not(target_os = "windows"))]
                            true,
                            #[cfg(target_os = "windows")]
                            false,
                        );

                        pipeline.add_back(line_based_frame_decoder_handler);
                        pipeline.add_back(string_codec_handler);
                        pipeline.add_back(echo_handler);
                        pipeline.finalize()
                    }));

                    let client_addr = client.bind("127.0.0.1:0").await.unwrap();
                    let pipeline = client.connect(server_addr).await.unwrap();

                    for i in 0..ITER {
                        // write
                        pipeline.write(TaggedString {
                            now: Instant::now(),
                            transport: TransportContext {
                                local_addr: client_addr,
                                peer_addr: server_addr,
                                ecn: EcnCodepoint::from_bits(1),
                                protocol: Protocol::UDP,
                            },
                            message: format!("{}\r\n", i),
                        });
                        yield_local();
                    }
                    pipeline.write(TaggedString {
                        now: Instant::now(),
                        transport: TransportContext {
                            local_addr: client_addr,
                            peer_addr: server_addr,
                            ecn: EcnCodepoint::from_bits(1),
                            protocol: Protocol::UDP,
                        },
                        message: format!("bye\r\n"),
                    });
                    yield_local();

                    assert!(client_done_rx.recv().await.is_some());

                    assert!(tx.send(client_count).is_ok());

                    client.graceful_stop().await;
                })
                .detach();

                let client_count = rx.recv().await.unwrap();
                assert!(server_done_rx.recv().await.is_some());

                let (client_count, server_count) = (client_count.borrow(), server_count.borrow());
                assert_eq!(*client_count, *server_count);
                assert_eq!(ITER + 1, *client_count);

                server.graceful_stop().await;
            })
            .unwrap();

        handler.join().unwrap();
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_echo_tcp() {
        LocalExecutorBuilder::default().run(async {
            const ITER: usize = 100; //1024;

            let (tx, mut rx) = channel();

            let server_count = Rc::new(RefCell::new(0));
            let server_count_clone = server_count.clone();
            let (server_done_tx, mut server_done_rx) = channel();
            let server_done_tx = Rc::new(RefCell::new(Some(server_done_tx)));

            let mut server = BootstrapTcpServer::new();
            server.pipeline(Box::new(move || {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = TaggedStringCodec::new();
                let echo_handler = EchoHandler::new(
                    true,
                    Rc::clone(&server_done_tx),
                    Rc::clone(&server_count_clone),
                    false,
                );

                pipeline.add_back(line_based_frame_decoder_handler);
                pipeline.add_back(string_codec_handler);
                pipeline.add_back(echo_handler);
                pipeline.finalize()
            }));

            let server_addr = server.bind("127.0.0.1:0").await.unwrap();

            spawn_local(async move {
                let client_count = Rc::new(RefCell::new(0));
                let client_count_clone = client_count.clone();
                let (client_done_tx, mut client_done_rx) = channel();
                let client_done_tx = Rc::new(RefCell::new(Some(client_done_tx)));

                let mut client = BootstrapTcpClient::new();
                client.pipeline(Box::new(move || {
                    let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                    let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                        LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                    ));
                    let string_codec_handler = TaggedStringCodec::new();
                    let echo_handler = EchoHandler::new(
                        false,
                        Rc::clone(&client_done_tx),
                        Rc::clone(&client_count_clone),
                        false,
                    );

                    pipeline.add_back(line_based_frame_decoder_handler);
                    pipeline.add_back(string_codec_handler);
                    pipeline.add_back(echo_handler);
                    pipeline.finalize()
                }));

                let client_addr = SocketAddr::from_str("127.0.0.1:0").unwrap();

                let pipeline = client.connect(server_addr).await.unwrap();

                for i in 0..ITER {
                    // write
                    pipeline.write(TaggedString {
                        now: Instant::now(),
                        transport: TransportContext {
                            local_addr: client_addr,
                            peer_addr: server_addr,
                            ecn: None,
                            protocol: Protocol::TCP,
                        },
                        message: format!("{}\r\n", i),
                    });
                    yield_local();
                }
                pipeline.write(TaggedString {
                    now: Instant::now(),
                    transport: TransportContext {
                        local_addr: client_addr,
                        peer_addr: server_addr,
                        ecn: None,
                        protocol: Protocol::TCP,
                    },
                    message: format!("bye\r\n"),
                });
                yield_local();

                assert!(client_done_rx.recv().await.is_some());

                assert!(tx.send(client_count).is_ok());

                client.graceful_stop().await;
            })
            .detach();

            let client_count = rx.recv().await.unwrap();
            assert!(server_done_rx.recv().await.is_some());

            let (client_count, server_count) = (client_count.borrow(), server_count.borrow());
            assert_eq!(*client_count, *server_count);
            assert_eq!(ITER + 1, *client_count);

            server.graceful_stop().await;
        });
    }
}
