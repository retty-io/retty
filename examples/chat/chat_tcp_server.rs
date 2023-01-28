use async_trait::async_trait;
use bytes::BytesMut;
use clap::Parser;
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use retty::bootstrap::BootstrapTcpServer;
use retty::channel::{
    Handler, InboundHandler, InboundHandlerContext, InboundHandlerInternal, OutboundHandler,
    OutboundHandlerContext, OutboundHandlerInternal, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{ByteToMessageCodec, LineBasedFrameDecoder, TerminatorType},
    string_codec::StringCodec,
};
use retty::runtime::{default_runtime, sync::Mutex};
use retty::transport::{AsyncTransportTcp, AsyncTransportWrite};

////////////////////////////////////////////////////////////////////////////////////////////////////
struct Shared {
    peers: HashMap<SocketAddr, Arc<Pipeline<BytesMut, String>>>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
        }
    }

    fn join(&mut self, peer: SocketAddr, pipeline: Arc<Pipeline<BytesMut, String>>) {
        println!("{} joined", peer);
        self.peers.insert(peer, pipeline);
    }

    fn leave(&mut self, peer: &SocketAddr) {
        println!("{} left", peer);
        self.peers.remove(peer);
    }

    /// Send message to every peer, except for the sender.
    async fn broadcast(&mut self, sender: SocketAddr, message: String) {
        print!("broadcast message: {}", message);
        for (peer, pipeline) in self.peers.iter() {
            if *peer != sender {
                let _ = pipeline.write(message.clone()).await;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
struct ChatDecoder {
    peer: SocketAddr,
    state: Arc<Mutex<Shared>>,
}
struct ChatEncoder;
struct ChatHandler {
    decoder: ChatDecoder,
    encoder: ChatEncoder,
}

impl ChatHandler {
    fn new(peer: SocketAddr, state: Arc<Mutex<Shared>>) -> Self {
        ChatHandler {
            decoder: ChatDecoder { peer, state },
            encoder: ChatEncoder,
        }
    }
}

#[async_trait]
impl InboundHandler for ChatDecoder {
    type Rin = String;
    type Rout = Self::Rin;

    async fn read(
        &mut self,
        _ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        msg: Self::Rin,
    ) {
        println!("received: {}", msg);

        let mut s = self.state.lock().await;
        s.broadcast(self.peer, format!("{}\r\n", msg)).await;
    }
    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        // first leave itself from state, otherwise, it may still receive message from broadcast,
        // which may cause data racing.
        {
            let mut s = self.state.lock().await;
            s.leave(&self.peer);
        }
        ctx.fire_close().await;
    }
}

#[async_trait]
impl OutboundHandler for ChatEncoder {
    type Win = String;
    type Wout = Self::Win;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        msg: Self::Win,
    ) {
        ctx.fire_write(msg).await;
    }
}

impl Handler for ChatHandler {
    type Rin = String;
    type Rout = Self::Rin;
    type Win = String;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "ChatHandler"
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let inbound_handler: Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>> =
            Box::new(self.decoder);
        let outbound_handler: Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>> =
            Box::new(self.encoder);

        (
            Arc::new(Mutex::new(inbound_handler)),
            Arc::new(Mutex::new(outbound_handler)),
        )
    }
}

#[derive(Parser)]
#[command(name = "Chat TCP Server")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of chat tcp server", long_about = None)]
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

    let mut bootstrap = BootstrapTcpServer::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            let state = state.clone();
            Box::pin(async move {
                let pipeline = Pipeline::new();

                let peer = sock.peer_addr().unwrap();
                let chat_handler = ChatHandler::new(peer, state.clone());
                let async_transport_handler = AsyncTransportTcp::new(sock);
                let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = StringCodec::new();

                pipeline.add_back(async_transport_handler).await;
                pipeline.add_back(line_based_frame_decoder_handler).await;
                pipeline.add_back(string_codec_handler).await;
                pipeline.add_back(chat_handler).await;
                pipeline.finalize().await;

                let pipeline = Arc::new(pipeline);
                {
                    let mut s = state.lock().await;
                    s.join(peer, pipeline.clone());
                }
                pipeline
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
