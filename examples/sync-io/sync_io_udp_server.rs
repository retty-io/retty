use clap::Parser;
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Sender;

use retty::bootstrap::BootstrapUdpServer;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::runtime::default_runtime;
use retty::transport::{AsyncTransport, TaggedBytesMut, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct TaggedSyncIODecoder {
    interval: Duration,
    timeout: Instant,
    last_transport: Option<TransportContext>,
}
struct TaggedSyncIOEncoder;
struct TaggedSyncIOHandler {
    decoder: TaggedSyncIODecoder,
    encoder: TaggedSyncIOEncoder,
}

impl TaggedSyncIOHandler {
    fn new(interval: Duration) -> Self {
        TaggedSyncIOHandler {
            decoder: TaggedSyncIODecoder {
                timeout: Instant::now() + interval,
                interval,
                last_transport: None,
            },
            encoder: TaggedSyncIOEncoder,
        }
    }
}

impl InboundHandler for TaggedSyncIODecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "handling {} from {:?}",
            msg.message, msg.transport.peer_addr
        );
        if msg.message == "bye" {
            self.last_transport.take();
        } else {
            self.last_transport = Some(msg.transport);
            ctx.fire_write(TaggedString {
                transport: msg.transport,
                message: format!("{}\r\n", msg.message),
            });
        }
    }

    fn read_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        if self.last_transport.is_some() && self.timeout <= now {
            println!("TaggedSyncIOHandler timeout at: {:?}", self.timeout);
            self.timeout = now + self.interval;
            if let Some(transport) = &self.last_transport {
                ctx.fire_write(TaggedString {
                    transport: *transport,
                    message: format!("Keep-alive message: next one at {:?}\r\n", self.timeout),
                });
            }
        }

        //last handler, no need to fire_read_timeout
    }
    fn poll_timeout(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, eto: &mut Instant) {
        if self.last_transport.is_some() && self.timeout < *eto {
            *eto = self.timeout;
        }

        //last handler, no need to fire_poll_timeout
    }
}

impl OutboundHandler for TaggedSyncIOEncoder {
    type Win = TaggedString;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg);
    }
}

impl Handler for TaggedSyncIOHandler {
    type Rin = TaggedString;
    type Rout = Self::Rin;
    type Win = TaggedString;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "TaggedSyncIOHandler"
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
#[command(name = "SyncIO UDP Server")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of sync-io udp server", long_about = None)]
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

    let mut bootstrap = BootstrapUdpServer::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(move |writer: Sender<TaggedBytesMut>| {
        Box::pin(async move {
            let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

            let async_transport_handler = AsyncTransport::new(writer);
            let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
            ));
            let string_codec_handler = TaggedStringCodec::new();
            let sync_io_handler = TaggedSyncIOHandler::new(Duration::from_secs(5));

            pipeline.add_back(async_transport_handler).await;
            pipeline.add_back(line_based_frame_decoder_handler).await;
            pipeline.add_back(string_codec_handler).await;
            pipeline.add_back(sync_io_handler).await;
            pipeline.finalize().await;

            Arc::new(pipeline)
        })
    }));

    bootstrap.bind(format!("{}:{}", host, port)).await?;

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            bootstrap.stop().await;
        }
    };

    Ok(())
}
