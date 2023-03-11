use clap::Parser;
use futures::StreamExt;
use local_sync::mpsc::unbounded::Tx;
use std::{error::Error, io::Write, net::SocketAddr, str::FromStr, time::Instant};

use retty::bootstrap::BootstrapClientTcp;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::transport::{AsyncTransport, TaggedBytesMut, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct EchoDecoder;
struct EchoEncoder;
struct EchoHandler {
    decoder: EchoDecoder,
    encoder: EchoEncoder,
}

impl EchoHandler {
    fn new() -> Self {
        EchoHandler {
            decoder: EchoDecoder,
            encoder: EchoEncoder,
        }
    }
}

impl InboundHandler for EchoDecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    fn read(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "received back: {} from {}",
            msg.message, msg.transport.peer_addr
        );
    }
    fn read_exception(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, err: Box<dyn Error>) {
        println!("received exception: {}", err);
        ctx.fire_close();
    }
    fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        println!("EOF received :(");
        ctx.fire_close();
    }
    fn poll_timeout(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, _eto: &mut Instant) {
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

#[derive(Parser)]
#[command(name = "Echo Client TCP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo client tcp", long_about = None)]
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

    println!("Connecting {}:{}...", host, port);

    let transport = TransportContext {
        local_addr: SocketAddr::from_str("0.0.0.0:0")?,
        peer_addr: SocketAddr::from_str(&format!("{}:{}", host, port))?,
        ecn: None,
    };

    let mut bootstrap = BootstrapClientTcp::new();
    bootstrap.pipeline(Box::new(move |writer: Tx<TaggedBytesMut>| {
        let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

        let async_transport_handler = AsyncTransport::new(writer);
        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
            LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
        ));
        let string_codec_handler = TaggedStringCodec::new();
        let echo_handler = EchoHandler::new();

        pipeline.add_back(async_transport_handler);
        pipeline.add_back(line_based_frame_decoder_handler);
        pipeline.add_back(string_codec_handler);
        pipeline.add_back(echo_handler);
        pipeline.finalize()
    }));

    let pipeline = bootstrap.connect(transport.peer_addr).await?;

    let (mut tx, mut rx) = futures::channel::mpsc::channel(8);
    std::thread::spawn(move || {
        let mut buffer = String::new();
        while std::io::stdin().read_line(&mut buffer).is_ok() {
            match buffer.trim_end() {
                "" => break,
                line => {
                    if tx.try_send(line.to_string()).is_err() {
                        break;
                    }
                    if line == "bye" {
                        break;
                    }
                }
            };
            buffer.clear();
        }
    });
    while let Some(line) = rx.next().await {
        pipeline.write(TaggedString {
            now: Instant::now(),
            transport,
            message: format!("{}\r\n", line),
        });
        if line == "bye" {
            pipeline.close();
            break;
        }
    }

    bootstrap.stop().await;

    Ok(())
}
