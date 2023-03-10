use clap::Parser;
use local_sync::mpsc::unbounded::Tx;
use std::{
    io::Write,
    rc::Rc,
    str::FromStr,
    time::{Duration, Instant},
};

use retty::bootstrap::BootstrapServerUdp;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::transport::{AsyncTransport, TaggedBytesMut};

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

    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "handling {} from {:?}",
            msg.message, msg.transport.peer_addr
        );
        if msg.message != "bye" {
            ctx.fire_write(TaggedString {
                now: Instant::now(),
                transport: msg.transport,
                message: format!("{}\r\n", msg.message),
            });
        }
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
#[command(name = "Echo Server UDP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo server udp", long_about = None)]
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

    let mut bootstrap = BootstrapServerUdp::new();
    bootstrap.pipeline(Box::new(move |writer: Tx<TaggedBytesMut>| {
        let mut pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

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
        Rc::new(pipeline.finalize())
    }));

    bootstrap.bind(format!("{}:{}", host, port))?;

    //TODO: https://github.com/bytedance/monoio/issues/154
    println!("Press ctrl-c to stop or wait 60s timout");
    println!("try `nc -u {} {}` in another shell", host, port);
    monoio::time::sleep(Duration::from_secs(60)).await;
    bootstrap.stop().await;

    Ok(())
}
