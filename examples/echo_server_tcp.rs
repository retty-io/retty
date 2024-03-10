use clap::Parser;
use std::collections::VecDeque;
use std::{io::Write, str::FromStr, time::Instant};

use retty::bootstrap::BootstrapTcpServer;
use retty::channel::{Context, Handler, Pipeline};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::TaggedStringCodec,
};
use retty::executor::LocalExecutorBuilder;
use retty::transport::{TaggedBytesMut, TaggedString};

////////////////////////////////////////////////////////////////////////////////////////////////////
struct EchoHandler {
    transmits: VecDeque<TaggedString>,
}

impl EchoHandler {
    fn new() -> Self {
        Self {
            transmits: VecDeque::new(),
        }
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
            "handling {} from {:?}",
            msg.message, msg.transport.peer_addr
        );
        self.transmits.push_back(TaggedString {
            now: Instant::now(),
            transport: msg.transport,
            message: format!("{}\r\n", msg.message),
        });
    }
    fn handle_read_eof(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_close();
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
        if let Some(msg) = ctx.fire_poll_write() {
            self.transmits.push_back(msg);
        }
        self.transmits.pop_front()
    }
}

#[derive(Parser)]
#[command(name = "Echo Server TCP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo server tcp", long_about = None)]
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

    println!("listening {}:{}...", host, port);

    LocalExecutorBuilder::default().run(async move {
        let mut bootstrap = BootstrapTcpServer::new();
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

        bootstrap.bind(format!("{}:{}", host, port)).await.unwrap();

        println!("Press ctrl-c to stop");
        println!("try `nc {} {}` in another shell", host, port);
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let mut tx = Some(tx);
            ctrlc::set_handler(move || {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
            })
            .expect("Error setting Ctrl-C handler");
        });
        let _ = rx.await;

        bootstrap.graceful_stop().await;
    });

    Ok(())
}
