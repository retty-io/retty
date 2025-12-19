use bytes::BytesMut;
use clap::Parser;
use log::{error, info, trace, warn};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::Write,
    net::SocketAddr,
    rc::Rc,
    rc::Weak,
    str::FromStr,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use wg::AsyncWaitGroup;

use sansio::{Context, Handler, RcInboundPipeline, RcOutboundPipeline, RcPipeline};
use sansio_executor::{LocalExecutorBuilder, spawn_local};

use sansio_codec::{
    LineBasedFrameDecoder, TaggedByteToMessageCodec, TaggedStringCodec, TerminatorType,
};
use sansio_transport::{TaggedBytesMut, TaggedString, TransportContext, TransportProtocol};

////////////////////////////////////////////////////////////////////////////////////////////////////
struct Shared {
    peers: HashMap<SocketAddr, Weak<dyn RcOutboundPipeline<TaggedString>>>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
        }
    }

    fn contains(&self, peer: &SocketAddr) -> bool {
        self.peers.contains_key(peer)
    }

    fn join(&mut self, peer: SocketAddr, pipeline: Weak<dyn RcOutboundPipeline<TaggedString>>) {
        info!("{} joined", peer);
        self.peers.insert(peer, pipeline);
    }

    fn leave(&mut self, peer: &SocketAddr) {
        info!("{} left", peer);
        self.peers.remove(peer);
    }

    /// Send message to every peer, except for the sender.
    fn broadcast(&self, sender: SocketAddr, msg: TaggedString) {
        print!("broadcast message: {}", msg.message);
        for (peer, pipeline) in self.peers.iter() {
            if *peer != sender {
                if let Some(pipeline) = pipeline.upgrade() {
                    let _ = pipeline.write(TaggedString {
                        now: msg.now,
                        transport: TransportContext {
                            local_addr: msg.transport.local_addr,
                            peer_addr: *peer,
                            ecn: msg.transport.ecn,
                            transport_protocol: msg.transport.transport_protocol,
                        },
                        message: msg.message.clone(),
                    });
                }
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
struct ChatHandler {
    state: Rc<RefCell<Shared>>,
    pipeline: Weak<dyn RcOutboundPipeline<TaggedString>>,
    peer_addr: Option<SocketAddr>,
}

impl ChatHandler {
    fn new(
        state: Rc<RefCell<Shared>>,
        pipeline: Weak<dyn RcOutboundPipeline<TaggedString>>,
    ) -> Self {
        ChatHandler {
            state,
            pipeline,
            peer_addr: None,
        }
    }
}

impl Handler for ChatHandler {
    type Rin = TaggedString;
    type Rout = Self::Rin;
    type Win = TaggedString;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "ChatHandler"
    }

    fn handle_read(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    ) {
        let peer_addr = msg.transport.peer_addr;
        info!(
            "received: {} from {:?} to {}",
            msg.message, peer_addr, msg.transport.local_addr
        );

        let mut s = self.state.borrow_mut();
        if !s.contains(&peer_addr) {
            s.join(peer_addr, self.pipeline.clone());
            self.peer_addr = Some(peer_addr);
        }
        s.broadcast(
            peer_addr,
            TaggedString {
                now: Instant::now(),
                transport: TransportContext {
                    local_addr: msg.transport.local_addr,
                    ecn: msg.transport.ecn,
                    ..Default::default()
                },
                message: format!("{}\r\n", msg.message),
            },
        );
    }

    fn handle_eof(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        // first leave itself from state, otherwise, it may still receive message from broadcast,
        // which may cause data racing.
        if let Some(peer_addr) = self.peer_addr {
            let mut s = self.state.borrow_mut();
            s.leave(&peer_addr);
        }
        ctx.fire_handle_close();
    }

    fn handle_timeout(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        _now: Instant,
    ) {
        //last handler, no need to fire_handle_timeout
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
        ctx.fire_poll_write()
    }
}

#[derive(Parser)]
#[command(name = "Chat Server TCP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.0.0")]
#[command(about = "An example of chat server tcp", long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,
    #[arg(long, default_value_t = format!("0.0.0.0"))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
    #[arg(long, default_value_t = format!("INFO"))]
    log_level: String,
}

fn build_pipeline(state: Rc<RefCell<Shared>>) -> Rc<RcPipeline<TaggedBytesMut, TaggedString>> {
    Rc::new_cyclic(|pipeline_weak| {
        let mut pipeline = RcPipeline::new();

        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
            LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
        ));
        let string_codec_handler = TaggedStringCodec::new();
        let chat_handler = ChatHandler::new(
            state,
            pipeline_weak.clone() as Weak<dyn RcOutboundPipeline<TaggedString>>,
        );

        pipeline.add_back(line_based_frame_decoder_handler);
        pipeline.add_back(string_codec_handler);
        pipeline.add_back(chat_handler);
        pipeline.finalize();

        pipeline
    })
}

async fn process_pipeline(
    mut stream: TcpStream,
    mut stop_rx: broadcast::Receiver<()>,
    state: Rc<RefCell<Shared>>,
) -> anyhow::Result<()> {
    let mut buf = vec![0; 2000];

    let pipeline = build_pipeline(state);
    pipeline.transport_active();
    loop {
        // prioritize stream.write than stream.read
        while let Some(transmit) = pipeline.poll_write() {
            match stream.write(&transmit.message).await {
                Ok(n) => {
                    trace!("stream write {} bytes", n);
                }
                Err(err) => {
                    warn!("stream write error {}", err);
                    break;
                }
            }
        }

        // Poll pipeline to get next timeout
        let mut eto = Instant::now() + Duration::from_millis(100);
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
            _ = stop_rx.recv() => {
                trace!("pipeline stream exit loop");
                break;
            }
            _ = timer.as_mut() =>{
                pipeline.handle_timeout(Instant::now());
            }
            res = stream.read(&mut buf) => {
                match res {
                    Ok(n) => {
                        if n == 0 {
                            pipeline.handle_eof();
                            break;
                        }

                        trace!("stream read {} bytes", n);
                        pipeline.handle_read(TaggedBytesMut {
                                now: Instant::now(),
                                transport: TransportContext {
                                    local_addr: stream.local_addr()?,
                                    peer_addr: stream.peer_addr()?,
                                    transport_protocol: TransportProtocol::TCP,
                                    ecn: None,
                                },
                                message: BytesMut::from(&buf[..n]),
                            });
                    }
                    Err(err) => {
                        warn!("stream read error {}", err);
                        break;
                    }
                }
            }
        }
    }
    pipeline.transport_inactive();

    trace!(
        "tcp connection on {} is gracefully down",
        stream.peer_addr()?
    );

    Ok(())
}

async fn run(mut stop_rx: broadcast::Receiver<()>, host: String, port: u16) -> anyhow::Result<()> {
    let wait_group = AsyncWaitGroup::new();

    // Create the shared state. This is how all the peers communicate.
    // The server task will hold a handle to this. For every new client, the
    // `state` handle is cloned and passed into the handler that processes the
    // client connection.
    let state = Rc::new(RefCell::new(Shared::new()));

    let listener = TcpListener::bind(format!("{host}:{port}")).await?;

    loop {
        tokio::select! {
            _ = stop_rx.recv() => {
                trace!("listener exit loop");
                break;
            }
            res = listener.accept() => {
                match res {
                    Ok((stream, addr)) => {
                        info!("Connection from {addr}");
                        let worker = wait_group.add(1);
                        let stream_stop_rx = stop_rx.resubscribe();
                        let state_clone = state.clone();
                        spawn_local( async move {
                            if let Err(err) = process_pipeline(stream, stream_stop_rx, state_clone).await {
                                error!("process_pipeline got error: {}", err);
                            }
                            worker.done();
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

    info!("Wait for Gracefully Shutdown...");
    wait_group.wait().await;
    info!("Server is Gracefully Shutdown Completed");

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let host = cli.host;
    let port = cli.port;
    let log_level = log::LevelFilter::from_str(&cli.log_level)?;
    if cli.debug {
        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} [{}] {} - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    chrono::Local::now().format("%H:%M:%S.%6f"),
                    record.args()
                )
            })
            .filter(None, log_level)
            .init();
    }

    let (stop_tx, stop_rx) = broadcast::channel::<()>(1);

    info!("Press Ctrl-C to stop");
    info!("try `nc {} {}` in another shell", host, port);
    std::thread::spawn(move || {
        let mut stop_tx = Some(stop_tx);
        ctrlc::set_handler(move || {
            if let Some(stop_tx) = stop_tx.take() {
                let _ = stop_tx.send(());
            }
        })
        .expect("Error setting Ctrl-C handler");
    });

    LocalExecutorBuilder::default().run(async move {
        if let Err(err) = run(stop_rx, host, port).await {
            error!("run got error: {}", err);
        }
    });

    Ok(())
}
