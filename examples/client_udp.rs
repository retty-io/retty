use clap::Parser;
use futures::StreamExt;
use std::{io::Write, net::SocketAddr, str::FromStr, time::Instant};

use retty::bootstrap::BootstrapUdpClient;
use retty::channel::{Context, Handler, Pipeline};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::TaggedStringCodec,
};
use retty::executor::LocalExecutorBuilder;
use retty::transport::{Protocol, TaggedBytesMut, TaggedString, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////
struct EchoHandler;

impl EchoHandler {
    fn new() -> Self {
        EchoHandler {}
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

    fn handle_read(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    ) {
        println!(
            "received back: {} from {:?}",
            msg.message, msg.transport.peer_addr
        );
    }
    fn poll_timeout(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        _eto: &mut Instant,
    ) {
        //last handler, no need to fire_poll_timeout
    }

    fn poll_write(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout> {
        ctx.fire_poll_write()
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

fn main() -> anyhow::Result<()> {
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
        protocol: Protocol::UDP,
    };

    LocalExecutorBuilder::default().run(async move {
        let mut bootstrap = BootstrapUdpClient::new();
        bootstrap.pipeline(Box::new(move || {
            let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

            let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
            ));
            let string_codec_handler = TaggedStringCodec::new();
            let echo_handler = EchoHandler::new();

            pipeline.add_back(line_based_frame_decoder_handler);
            pipeline.add_back(string_codec_handler);
            pipeline.add_back(echo_handler);
            pipeline.finalize()
        }));

        bootstrap.bind(transport.local_addr).await.unwrap();

        let pipeline = bootstrap.connect(transport.peer_addr).await.unwrap();

        println!("Enter bye to stop");
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

        bootstrap.graceful_stop().await;
    });

    Ok(())
}
