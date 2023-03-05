use bytes::BytesMut;
use clap::Parser;
use retty_io::channel::Sender;
use std::{
    error::Error,
    io::{stdin, Write},
    str::FromStr,
    time::{Duration, Instant},
};

use retty::bootstrap::BootstrapClientTcp;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{ByteToMessageCodec, LineBasedFrameDecoder, TerminatorType},
    string_codec::StringCodec,
};
use retty::transport::AsyncTransport;

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

impl InboundHandler for EchoDecoder {
    type Rin = String;
    type Rout = Self::Rin;

    fn read(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!("received back: {}", msg);
    }
    fn read_exception(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, err: Box<dyn Error>) {
        println!("received exception: {}", err);
        ctx.fire_close();
    }

    fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        println!("EOF received :(");
        ctx.fire_close();
    }

    fn handle_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        if now >= self.timeout {
            println!("EchoHandler timeout at: {:?}", self.timeout);
            self.interval += Duration::from_secs(1);
            self.timeout = now + self.interval;
            ctx.fire_write(format!(
                "Keep-alive message: next one for interval {:?}\r\n",
                self.interval
            ));
        }

        //last handler, no need to fire_handle_timeout
    }

    fn poll_timeout(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, eto: &mut Instant) {
        if self.timeout < *eto {
            *eto = self.timeout
        }

        //last handler, no need to fire_poll_timeout
    }
}

impl OutboundHandler for EchoEncoder {
    type Win = String;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg);
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
#[command(name = "Echo Client TCP")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo client tcp", long_about = None)]
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

    let mut bootstrap = BootstrapClientTcp::new();
    bootstrap.pipeline(Box::new(move |writer: Sender<BytesMut>| {
        let pipeline: Pipeline<BytesMut, String> = Pipeline::new();

        let async_transport_handler = AsyncTransport::new(writer);
        let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
            LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
        ));
        let string_codec_handler = StringCodec::new();
        let echo_handler = EchoHandler::new(Duration::from_secs(10));

        pipeline.add_back(async_transport_handler);
        pipeline.add_back(line_based_frame_decoder_handler);
        pipeline.add_back(string_codec_handler);
        pipeline.add_back(echo_handler);
        pipeline.finalize()
    }));

    let pipeline = bootstrap.connect(format!("{}:{}", host, port))?;

    println!("Enter bye to stop");
    let mut buffer = String::new();
    while stdin().read_line(&mut buffer).is_ok() {
        match buffer.trim_end() {
            "" => break,
            line => {
                pipeline.write(format!("{}\r\n", line));
                if line == "bye" {
                    pipeline.close();
                    break;
                }
            }
        };
        buffer.clear();
    }

    bootstrap.stop();

    Ok(())
}
