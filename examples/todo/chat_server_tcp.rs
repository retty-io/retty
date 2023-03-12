use clap::Parser;
use local_sync::mpsc::unbounded::Tx;
use std::{
    cell::RefCell, collections::HashMap, io::Write, net::SocketAddr, rc::Rc, rc::Weak,
    str::FromStr, time::Instant,
};

use retty::bootstrap::BootstrapTcpServer;
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
    peers: HashMap<SocketAddr, Weak<dyn OutboundPipeline<TaggedString>>>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
        }
    }

    fn join(&mut self, peer: SocketAddr, pipeline: Weak<dyn OutboundPipeline<TaggedString>>) {
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
                if let Some(pipeline) = pipeline.upgrade() {
                    let _ = pipeline.write(msg);
                }
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
struct ChatDecoder {
    state: Rc<RefCell<Shared>>,
    pipeline: Weak<dyn OutboundPipeline<TaggedString>>,
    peer: Option<SocketAddr>,
}
struct ChatEncoder;
struct ChatHandler {
    decoder: ChatDecoder,
    encoder: ChatEncoder,
}

impl ChatHandler {
    fn new(state: Rc<RefCell<Shared>>, pipeline: Weak<dyn OutboundPipeline<TaggedString>>) -> Self {
        ChatHandler {
            decoder: ChatDecoder {
                state,
                pipeline,
                peer: None,
            },
            encoder: ChatEncoder,
        }
    }
}

impl InboundHandler for ChatDecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    fn read(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "received: {} from {} to {}",
            msg.message, msg.transport.peer_addr, msg.transport.local_addr
        );

        if self.peer.is_none() {
            self.peer = Some(msg.transport.peer_addr);
            let mut s = self.state.borrow_mut();
            s.join(msg.transport.peer_addr, self.pipeline.clone());
        }

        let s = self.state.borrow();
        s.broadcast(
            msg.transport.peer_addr,
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
    fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        // first leave itself from state, otherwise, it may still receive message from broadcast,
        // which may cause data racing.
        if let Some(peer) = &self.peer {
            let mut s = self.state.borrow_mut();
            s.leave(peer);
        }
        ctx.fire_close();
    }
    fn poll_timeout(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, _eto: &mut Instant) {
        //last handler, no need to fire_poll_timeout
    }
}

impl OutboundHandler for ChatEncoder {
    type Win = TaggedString;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg);
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
#[command(name = "Chat Server TCP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
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

#[monoio::main(driver = "fusion", enable_timer = true)]
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
    let state = Rc::new(RefCell::new(Shared::new()));

    let mut bootstrap = BootstrapTcpServer::new();
    bootstrap.pipeline(Box::new(move |writer: Tx<TaggedBytesMut>| {
        let pipeline: Rc<Pipeline<TaggedBytesMut, TaggedString>> = Rc::new(Pipeline::new());

        let async_transport_handler = AsyncTransport::new(writer);
        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
            LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
        ));
        let string_codec_handler = TaggedStringCodec::new();
        let pipeline_wr = Rc::downgrade(&pipeline);
        let chat_handler = ChatHandler::new(state.clone(), pipeline_wr);

        pipeline.add_back(async_transport_handler);
        pipeline.add_back(line_based_frame_decoder_handler);
        pipeline.add_back(string_codec_handler);
        pipeline.add_back(chat_handler);
        pipeline.update()
    }));

    bootstrap.bind(format!("{}:{}", host, port))?;

    println!("Press ctrl-c to stop");
    println!("try `nc {} {}` in another shell", host, port);
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let mut tx = Some(tx);
        ctrlc::set_handler(move || {
            if let Some(tx) = tx.take() {
                let _ = tx.send(());
            }
        })
        .expect("Error setting Ctrl-C handler");
    });
    let _ = rx.await;

    bootstrap.stop().await;

    Ok(())
}
