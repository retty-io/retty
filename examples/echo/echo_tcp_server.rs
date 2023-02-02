use async_trait::async_trait;
use bytes::BytesMut;
use clap::Parser;
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;

use retty::bootstrap::BootstrapTcpServer;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{ByteToMessageCodec, LineBasedFrameDecoder, TerminatorType},
    string_codec::StringCodec,
};
use retty::runtime::default_runtime;
use retty::transport::{AsyncTransportTcp, AsyncTransportWrite};

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

#[async_trait]
impl InboundHandler for EchoDecoder {
    type Rin = String;
    type Rout = Self::Rin;

    async fn read(&mut self, ctx: &mut InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!("handling {}", msg);
        ctx.fire_write(format!("{}\r\n", msg)).await;
    }
    async fn read_eof(&mut self, ctx: &mut InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_close().await;
    }
}

#[async_trait]
impl OutboundHandler for EchoEncoder {
    type Win = String;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &mut OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg).await;
    }
}

impl Handler for EchoHandler {
    type Rin = String;
    type Rout = Self::Rin;
    type Win = String;
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
#[command(name = "Echo TCP Server")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo tcp server", long_about = None)]
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

    let mut bootstrap = BootstrapTcpServer::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<BytesMut, String> = Pipeline::new();

                let async_transport_handler = AsyncTransportTcp::new(sock);
                let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = StringCodec::new();
                let echo_handler = EchoHandler::new();

                pipeline.add_back(async_transport_handler).await;
                pipeline.add_back(line_based_frame_decoder_handler).await;
                pipeline.add_back(string_codec_handler).await;
                pipeline.add_back(echo_handler).await;
                pipeline.finalize().await;

                Arc::new(pipeline)
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
