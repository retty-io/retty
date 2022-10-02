use async_trait::async_trait;
use clap::{AppSettings, Arg, Command};
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Mutex;

use retty::bootstrap::bootstrap_udp_server::BootstrapUdpServer;
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
use retty::transport::AsyncTransportWrite;
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
    async fn read(&mut self, ctx: &mut InboundHandlerContext, message: Message) {
        let msg = message.body.downcast_ref::<String>().unwrap();
        println!("handling {}", msg);
        if msg == "close" {
            ctx.fire_close().await;
        } else {
            ctx.fire_write(Message {
                transport: message.transport,
                body: Box::new(format!("{}\r\n", msg)),
            })
            .await;
        }
    }

    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext) {
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
    let mut app = Command::new("Echo UDP Server")
        .version("0.1.0")
        .author("Rusty Rain <y@liu.mx>")
        .about("An example of echo udp server")
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

    println!("listening {}:{}...", host, port);

    let mut bootstrap = BootstrapUdpServer::new();
    bootstrap
        .pipeline(Box::new(
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
