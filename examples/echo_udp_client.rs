use async_trait::async_trait;
use clap::{AppSettings, Arg, Command};
use std::io::stdin;
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use retty::bootstrap::bootstrap_udp_client::BootstrapUdpClient;
use retty::channel::{
    handler::{Handler, InboundHandler, InboundHandlerContext, OutboundHandler},
    pipeline::Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::ByteToMessageCodec,
    line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType},
    string_codec::StringCodec,
};
use retty::error::Error;
use retty::transport::async_transport_udp::AsyncTransportUdp;
use retty::transport::{AsyncTransportWrite, TransportContext};
use retty::Message;

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
    async fn read(&mut self, _ctx: &mut InboundHandlerContext, message: Message) {
        let msg = message.body.downcast_ref::<String>().unwrap();
        println!("received back: {}", msg);
    }

    async fn read_exception(&mut self, ctx: &mut InboundHandlerContext, error: Error) {
        println!("received exception: {}", error);
        ctx.fire_close().await;
    }

    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext) {
        println!("EOF received :(");
        ctx.fire_close().await;
    }
}

#[async_trait]
impl OutboundHandler for EchoEncoder {}

impl Handler for EchoHandler {
    fn id(&self) -> String {
        "Echo Handler".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandler>>,
        Arc<Mutex<dyn OutboundHandler>>,
    ) {
        let (decoder, encoder) = (self.decoder, self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut app = Command::new("Echo UDP Client")
        .version("0.1.0")
        .author("Rusty Rain <y@liu.mx>")
        .about("An example of echo udp client")
        .setting(AppSettings::DeriveDisplayOrder)
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("FULLHELP")
                .help("Prints more detailed help information")
                .long("fullhelp"),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Prints debug log information"),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .short('h')
                .default_value("0.0.0.0")
                .help("Echo server address"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .default_value("8080")
                .help("Echo server port"),
        );

    let matches = app.clone().get_matches();

    if matches.is_present("FULLHELP") {
        app.print_long_help().unwrap();
        std::process::exit(0);
    }

    let host = matches.value_of("host").unwrap().to_owned();
    let port = matches.value_of("port").unwrap().to_owned();
    let debug = matches.is_present("debug");
    if debug {
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

    println!("Connecting {}:{}...", host, port);

    let transport = TransportContext {
        local_addr: SocketAddr::from_str("0.0.0.0:0")?,
        peer_addr: SocketAddr::from_str(&format!("{}:{}", host, port))?,
    };

    let mut client = BootstrapUdpClient::new();
    client.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            let mut pipeline = Pipeline::new();

            let async_transport_handler = AsyncTransportUdp::new(sock);
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
    ));

    client.bind(transport.local_addr).await?;

    let pipeline = client.connect(transport.peer_addr).await?;

    println!("Enter bye to stop");
    let mut buffer = String::new();
    while stdin().read_line(&mut buffer).is_ok() {
        match buffer.trim_end() {
            "" => break,
            line => {
                pipeline
                    .write(Message {
                        transport,
                        body: Box::new(format!("{}\r\n", line)),
                    })
                    .await;
                if line == "bye" {
                    pipeline.close().await;
                    break;
                }
            }
        };
        buffer.clear();
    }

    Ok(())
}
