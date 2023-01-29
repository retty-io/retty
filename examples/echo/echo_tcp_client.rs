use async_trait::async_trait;
use bytes::BytesMut;
use clap::Parser;
use std::error::Error;
use std::io::stdin;
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use retty::bootstrap::BootstrapTcpClient;
use retty::channel::{
    Handler, InboundHandler, InboundHandlerContext, OutboundHandler, OutboundHandlerContext,
    Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{ByteToMessageCodec, LineBasedFrameDecoder, TerminatorType},
    string_codec::StringCodec,
};
use retty::runtime::default_runtime;
use retty::transport::{AsyncTransportTcp, AsyncTransportWrite};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct EchoDecoder {
    interval: Duration,
    timeout: Instant,
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
            },
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
        _ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        msg: Self::Rin,
    ) {
        println!("received back: {}", msg);
    }
    async fn read_exception(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        err: Box<dyn Error + Send + Sync>,
    ) {
        println!("received exception: {}", err);
        ctx.fire_close().await;
    }
    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        println!("EOF received :(");
        ctx.fire_close().await;
    }

    async fn read_timeout(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        timeout: Instant,
    ) {
        if timeout >= self.timeout {
            println!("EchoHandler timeout at: {:?}", self.timeout);
            self.timeout = Instant::now() + self.interval;
            ctx.fire_write(format!(
                "Keep-alive message: next one at {:?}\r\n",
                self.timeout
            ))
            .await;
        }

        //last handler, no need to fire_read_timeout
    }
    async fn poll_timeout(
        &mut self,
        _ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        timeout: &mut Instant,
    ) {
        if self.timeout < *timeout {
            *timeout = self.timeout
        }

        //last handler, no need to fire_poll_timeout
    }
}

#[async_trait]
impl OutboundHandler for EchoEncoder {
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
#[command(name = "Echo TCP Client")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo tcp client", long_about = None)]
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

    println!("Connecting {}:{}...", host, port);

    let mut bootstrap = BootstrapTcpClient::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<BytesMut, String> = Pipeline::new();

                let async_transport_handler = AsyncTransportTcp::new(sock);
                let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = StringCodec::new();
                let echo_handler = EchoHandler::new(Duration::from_secs(5));

                pipeline.add_back(async_transport_handler).await;
                pipeline.add_back(line_based_frame_decoder_handler).await;
                pipeline.add_back(string_codec_handler).await;
                pipeline.add_back(echo_handler).await;
                pipeline.finalize().await;

                Arc::new(pipeline)
            })
        },
    ));

    let pipeline = bootstrap.connect(format!("{}:{}", host, port)).await?;

    println!("Enter bye to stop");
    let mut buffer = String::new();
    while stdin().read_line(&mut buffer).is_ok() {
        match buffer.trim_end() {
            "" => break,
            line => {
                if line == "bye" {
                    pipeline.close().await;
                    break;
                }
                pipeline.write(format!("{}\r\n", line)).await;
            }
        };
        buffer.clear();
    }

    Ok(())
}
