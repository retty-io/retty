use clap::Parser;
use local_sync::mpsc::unbounded::Tx;
use std::{
    io::Write,
    net::SocketAddr,
    rc::Rc,
    str::FromStr,
    time::{Duration, Instant},
};

use retty::bootstrap::BootstrapClientUdp;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::transport::{AsyncTransport, TaggedBytesMut, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct EchoDecoder {
    interval: Duration,
    timeout: Instant,
    transport: Option<TransportContext>,
}
struct EchoEncoder;
struct EchoHandler {
    decoder: EchoDecoder,
    encoder: EchoEncoder,
}

impl EchoHandler {
    fn new(interval: Duration) -> Self {
        EchoHandler {
            decoder: EchoDecoder {
                timeout: Instant::now() + interval,
                interval,
                transport: None,
            },
            encoder: EchoEncoder,
        }
    }
}

impl InboundHandler for EchoDecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    fn read(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "received back: {} from {:?}",
            msg.message, msg.transport.peer_addr
        );
        self.transport = Some(msg.transport);
    }
    fn handle_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        if self.timeout <= now {
            println!("EchoHandler timeout at: {:?}", self.timeout);
            self.interval += Duration::from_secs(1);
            self.timeout = now + self.interval;
            if let Some(transport) = &self.transport {
                ctx.fire_write(TaggedString {
                    now: Instant::now(),
                    transport: *transport,
                    message: format!(
                        "Keep-alive message: next one for interval {:?}\r\n",
                        self.interval
                    ),
                });
            }
        }

        //last handler, no need to fire_handle_timeout
    }
    fn poll_timeout(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, eto: &mut Instant) {
        if self.timeout < *eto {
            *eto = self.timeout;
        }

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
#[command(name = "Echo Client UDP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo client udp", long_about = None)]
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
        peer_addr: Some(SocketAddr::from_str(&format!("{}:{}", host, port))?),
        ecn: None,
    };

    let mut bootstrap = BootstrapClientUdp::new();
    bootstrap.pipeline(Box::new(move |writer: Tx<TaggedBytesMut>| {
        let mut pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

        let async_transport_handler = AsyncTransport::new(writer);
        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
            LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
        ));
        let string_codec_handler = TaggedStringCodec::new();
        let echo_handler = EchoHandler::new(Duration::from_secs(10));

        pipeline.add_back(async_transport_handler);
        pipeline.add_back(line_based_frame_decoder_handler);
        pipeline.add_back(string_codec_handler);
        pipeline.add_back(echo_handler);
        Rc::new(pipeline.finalize())
    }));

    bootstrap.bind(transport.local_addr)?;

    let pipeline = bootstrap
        .connect(*transport.peer_addr.as_ref().unwrap())
        .await?;

    /*TODO: https://github.com/bytedance/monoio/issues/155
    println!("Enter bye to stop");
    let mut buffer = String::new();
    while stdin().read_line(&mut buffer).is_ok() {
        match buffer.trim_end() {
            "" => break,
            line => {
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
        };
        buffer.clear();
    }*/

    monoio::time::timeout(Duration::from_secs(1), async {
        pipeline.write(TaggedString {
            now: Instant::now(),
            transport,
            message: format!("hello\r\n"),
        });
    })
    .await?;

    //TODO: https://github.com/bytedance/monoio/issues/154
    println!("Press ctrl-c to stop or wait 60s timout");
    println!("try `nc -u {} {}` in another shell", host, port);
    monoio::time::sleep(Duration::from_secs(60)).await;
    bootstrap.stop().await;

    Ok(())
}
