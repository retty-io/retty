use async_trait::async_trait;
use clap::Parser;
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;

use retty::bootstrap::bootstrap_tcp_server::BootstrapTcpServer;
use retty::channel::{
    handler::{
        Handler, InboundHandler, InboundHandlerContext, OutboundHandler, OutboundHandlerContext,
    },
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{
        line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType},
        ByteToMessageCodec,
    },
    string_codec::StringCodec,
};
use retty::runtime::{default_runtime, sync::Mutex};
use retty::transport::async_transport_tcp::AsyncTransportTcp;
use retty::transport::AsyncTransportWrite;

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

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        message: &mut Self::Rin,
    ) {
        println!("handling {}", message);
        ctx.fire_write(&mut format!("{}\r\n", message)).await;
    }
    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_close().await;
    }
}

#[async_trait]
impl OutboundHandler for EchoEncoder {
    type Win = String;
    type Wout = Self::Win;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        message: &mut Self::Win,
    ) {
        ctx.fire_write(message).await;
    }
}

impl Handler for EchoHandler {
    type In = String;
    type Out = Self::In;

    fn name(&self) -> &str {
        "EchoHandler"
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let inbound_handler: Box<dyn InboundHandler<Rin = Self::In, Rout = Self::Out>> =
            Box::new(self.decoder);
        let outbound_handler: Box<dyn OutboundHandler<Win = Self::Out, Wout = Self::In>> =
            Box::new(self.encoder);

        (
            Arc::new(Mutex::new(inbound_handler)),
            Arc::new(Mutex::new(outbound_handler)),
        )
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
    bootstrap
        .pipeline(Box::new(
            move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
                let mut pipeline = Pipeline::new();

                let async_transport_handler = AsyncTransportTcp::new(sock);
                let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = StringCodec::new();
                let echo_handler = EchoHandler::new();

                pipeline.add_back(async_transport_handler);
                pipeline.add_back(line_based_frame_decoder_handler);
                pipeline.add_back(string_codec_handler);
                pipeline.add_back(echo_handler);

                Box::pin(async move { pipeline.finalize().await })
            },
        ))
        .bind(format!("{}:{}", host, port))
        .await?;

    println!("Press ctrl-c to stop");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            bootstrap.stop().await;
        }
    };

    Ok(())
}
