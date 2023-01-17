use async_trait::async_trait;
use clap::Parser;
use std::io::Write;
use std::sync::Arc;

use retty::bootstrap::bootstrap_tcp_server::BootstrapTcpServer;
use retty::channel::{
    handler::{
        Handler, InboundHandlerContext, InboundHandlerGeneric, InboundHandlerInternal,
        OutboundHandlerGeneric, OutboundHandlerInternal,
    },
    pipeline::Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{
        line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType},
        ByteToMessageCodec,
    },
    string_codec::StringCodec,
};
use retty::error::Error;
use retty::runtime::{default_runtime, sync::Mutex};
use retty::transport::async_transport_tcp::AsyncTransportTcp;
use retty::transport::{AsyncTransportWrite, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct TelnetDecoder;
struct TelnetEncoder;
struct TelnetHandler {
    decoder: TelnetDecoder,
    encoder: TelnetEncoder,
}

impl TelnetHandler {
    fn new() -> Self {
        Self {
            decoder: TelnetDecoder,
            encoder: TelnetEncoder,
        }
    }
}

#[async_trait]
impl InboundHandlerGeneric<String> for TelnetDecoder {
    async fn read_generic(&mut self, ctx: &mut InboundHandlerContext, message: &mut String) {
        if message.is_empty() {
            ctx.fire_write(&mut "Please type something.\r\n".to_string())
                .await;
        } else if message == "bye" {
            ctx.fire_write(&mut "Have a fabulous day!\r\n".to_string())
                .await;
            ctx.fire_close().await;
        } else {
            ctx.fire_write(&mut format!("Did you say '{}'?\r\n", message))
                .await;
        }
    }

    async fn transport_active_generic(&mut self, ctx: &mut InboundHandlerContext) {
        let transport = ctx.get_transport();
        ctx.fire_write(&mut format!(
            "Welcome to {}!\r\nType 'bye' to disconnect.\r\n",
            transport.local_addr
        ))
        .await;
    }
}

impl OutboundHandlerGeneric<String> for TelnetEncoder {}

impl Handler for TelnetHandler {
    fn id(&self) -> String {
        "Telnet Handler".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    ) {
        let decoder: Box<dyn InboundHandlerGeneric<String>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandlerGeneric<String>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}

#[derive(Parser)]
#[command(name = "Telnet Server")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of telnet server", long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,
    #[arg(long, default_value_t = format!("0.0.0.0"))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    let host = cli.host;
    let port = cli.port;
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
            .filter(None, log::LevelFilter::Trace)
            .init();
    }

    println!("listening {}:{}...", host, port);

    let mut bootstrap = BootstrapTcpServer::new(default_runtime().unwrap());
    bootstrap
        .pipeline(Box::new(
            move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
                let mut pipeline = Pipeline::new(TransportContext {
                    local_addr: sock.local_addr().unwrap(),
                    peer_addr: sock.peer_addr().ok(),
                });

                let async_transport_handler = AsyncTransportTcp::new(sock);
                let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = StringCodec::new();
                let telnet_handler = TelnetHandler::new();

                pipeline.add_back(async_transport_handler);
                pipeline.add_back(line_based_frame_decoder_handler);
                pipeline.add_back(string_codec_handler);
                pipeline.add_back(telnet_handler);

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
