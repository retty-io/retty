use clap::Parser;
use local_sync::mpsc::unbounded::Tx;
use std::{cell::RefCell, collections::HashMap, io::Write, rc::Rc, str::FromStr, time::Instant};

use retty::bootstrap::BootstrapServerUdp;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, OutboundPipeline,
    Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::transport::{AsyncTransport, TaggedBytesMut, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////
struct Shared {
    peers: HashMap<SocketAddr, Rc<dyn OutboundPipeline>>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
        }
    }

    fn join(&mut self, peer: SocketAddr, pipeline: Rc<dyn OutboundPipeline>) {
        println!("{} joined", peer);
        self.peers.insert(peer, pipeline);
    }

    fn leave(&mut self, peer: &SocketAddr) {
        println!("{} left", peer);
        self.peers.remove(peer);
    }

    /// Send message to every peer, except for the sender.
    fn broadcast(&self, sender: SocketAddr, msg: TaggedString) {
        print!("broadcast message: {}", msg.message);
        for (peer, pipeline) in self.peers.iter() {
            if *peer != sender {
                let mut msg = msg.clone();
                msg.transport.peer_addr = *peer;
                let _ = pipeline.fire_write(msg);
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
struct ChatDecoder {
    state: Rc<RefCell<Shared>>,
}
struct ChatEncoder;
struct ChatHandler {
    decoder: ChatDecoder,
    encoder: ChatEncoder,
}

impl ChatHandler {
    fn new(state: Rc<RefCell<Shared>>) -> Self {
        ChatHandler {
            decoder: ChatDecoder { state },
            encoder: ChatEncoder,
        }
    }
}

#[async_trait]
impl InboundHandler for ChatDecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    async fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "received: {} from {:?} with ECN {:?} to {:?}",
            msg.message, msg.transport.peer_addr, msg.transport.ecn, msg.transport.local_addr
        );

        let peer = msg.transport.peer_addr.unwrap();

        let mut s = self.state.lock().await;
        if msg.message == "bye" {
            s.leave(&peer);
        } else {
            if !s.contains(&peer) {
                s.join(peer);
            }
            s.broadcast(
                ctx,
                peer,
                TaggedString {
                    now: Instant::now(),
                    transport: TransportContext {
                        local_addr: msg.transport.local_addr,
                        ecn: msg.transport.ecn,
                        ..Default::default()
                    },
                    message: format!("{}\r\n", msg.message),
                },
            )
            .await;
        }
    }
}

#[async_trait]
impl OutboundHandler for ChatEncoder {
    type Win = TaggedString;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg).await;
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

    fn split(
        self,
    ) -> (
        Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
        Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
    ) {
        (Box::new(self.decoder), Box::new(self.encoder))
    }
}

#[derive(Parser)]
#[command(name = "Chat Server UDP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of chat server udp", long_about = None)]
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    println!("listening {}:{}...", host, port);

    // Create the shared state. This is how all the peers communicate.
    // The server task will hold a handle to this. For every new client, the
    // `state` handle is cloned and passed into the handler that processes the
    // client connection.
    let state = Arc::new(Mutex::new(Shared::new()));

    let mut bootstrap = BootstrapServerUdpEcn::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            let state = state.clone();
            Box::pin(async move {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                let async_transport_handler = AsyncTransportUdpEcn::new(sock);
                let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = TaggedStringCodec::new();
                let chat_handler = ChatHandler::new(state);

                pipeline.add_back(async_transport_handler).await;
                pipeline.add_back(line_based_frame_decoder_handler).await;
                pipeline.add_back(string_codec_handler).await;
                pipeline.add_back(chat_handler).await;
                pipeline.finalize().await
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", host, port)).await?;

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            bootstrap.stop().await;
        }
    };

    Ok(())
}
