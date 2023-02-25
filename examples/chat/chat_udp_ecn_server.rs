use async_trait::async_trait;
use clap::Parser;
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use retty::bootstrap::BootstrapUdpEcnServer;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::runtime::{default_runtime, sync::Mutex};
use retty::transport::{
    AsyncTransportUdpEcn, AsyncTransportWrite, TaggedBytesMut, TransportContext,
};

////////////////////////////////////////////////////////////////////////////////////////////////////
struct Shared {
    peers: HashMap<SocketAddr, Arc<Pipeline<TaggedBytesMut, TaggedString>>>,
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

    /*TODO:fn join(&mut self, peer: SocketAddr, pipeline: Arc<Pipeline<TaggedBytesMut, TaggedString>>) {
        println!("{} joined", peer);
        self.peers.insert(peer, pipeline);
    }*/

    fn leave(&mut self, peer: &SocketAddr) {
        println!("{} left", peer);
        self.peers.remove(peer);
    }

    /// Send message to every peer, except for the sender.
    async fn broadcast(&mut self, sender: SocketAddr, message: TaggedString) {
        print!(
            "broadcast message: ECN {:?} and MSG {}",
            message.transport.ecn, message.message,
        );
        for (peer, pipeline) in self.peers.iter() {
            if *peer != sender {
                let mut msg = message.clone();
                msg.transport.peer_addr = Some(*peer);
                let _ = pipeline.write(msg).await;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
struct ChatDecoder {
    state: Arc<Mutex<Shared>>,
}
struct ChatEncoder;
struct ChatHandler {
    decoder: ChatDecoder,
    encoder: ChatEncoder,
}

impl ChatHandler {
    fn new(state: Arc<Mutex<Shared>>) -> Self {
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

    async fn read(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
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
                /*TODO:let pipeline: Weak<Pipeline<TaggedBytesMut, TaggedString>> = ctx.get_pipeline();
                if let Some(pipeline) = pipeline.upgrade() {
                    s.join(peer, pipeline);
                }*/
            }
            s.broadcast(
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
#[command(name = "Chat UDP Server with ECN")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of chat udp server with ECN", long_about = None)]
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

    let mut bootstrap = BootstrapUdpEcnServer::new(default_runtime().unwrap());
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
                let chat_handler = ChatHandler::new(state.clone());

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
