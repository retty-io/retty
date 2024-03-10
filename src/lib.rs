//! ### What is Retty?
//! Retty is an asynchronous Rust networking framework that makes it easy to build protocols, application clients/servers.
//!
//! It's like [Netty](https://netty.io) or [Wangle](https://github.com/facebook/wangle), but in Rust.
//!
//! ### What is a Pipeline?
//! The fundamental abstraction of Retty is the [Pipeline](crate::channel::Pipeline).
//! It offers immense flexibility to customize how requests and responses are handled by your service.
//! Once you have fully understood this abstraction,
//! you will be able to write all sorts of sophisticated protocols, application clients/servers.
//!  
//! A [Pipeline](crate::channel::Pipeline) is a chain of request/response [handlers](crate::channel::Handler) that handle inbound request and
//! outbound response. Once you chain handlers together, it provides an agile way to convert
//! a raw data stream into the desired message type and the inverse -- desired message type to raw data stream.
//! Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
//! over how an event is handled and how the handlers in a pipeline interact with each other.
//!
//! A [Handler](crate::channel::Handler) should do one and only one function - just like the UNIX philosophy. If you have a handler that
//! is doing more than one function than you should split it into individual handlers. This is really important for
//! maintainability and flexibility as its common to change your protocol for one reason or the other.
//!
//! ### How does an event flow in a Pipeline?
//! ```text
//!                                                       | write()
//!   +---------------------------------------------------+---------------+
//!   |                             Pipeline              |               |
//!   |                                                  \|/              |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |    |                       Handler  N                        |    |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |              /|\                                  |               |
//!   |               |                                   |               |
//!   |               |                                   |               |
//!   |               |                                  \|/              |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |    |                       Handler N-1                       |    |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |              /|\                                  |               |
//!   |               |                                   |               |
//!   |               |                         Context.fire_poll_write() |
//!   |               |                                   |               |
//!   |               |                                   |               |
//!   |          Context.fire_read()                      |               |
//!   |               |                                   |               |
//!   |               |                                  \|/              |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |    |                       Handler  2                        |    |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |              /|\                                  |               |
//!   |               |                                   |               |
//!   |               |                                   |               |
//!   |               |                                  \|/              |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |    |                       Handler  1                        |    |
//!   |    +----------+----------+------------+-----------+----------+    |
//!   |              /|\                                  |               |
//!   +---------------+-----------------------------------+---------------+
//!                   | read()                            | poll_write()
//!                   |                                  \|/
//!   +---------------+-----------------------------------+---------------+
//!   |               |                                   |               |
//!   |        Internal I/O Threads (Transport Implementation)            |
//!   +-------------------------------------------------------------------+
//! ```
//!
//! ### Echo Server Example
//! Let's look at how to write an echo server.
//!
//! Here's the main piece of code in our echo server; it receives a string from inbound direction in the pipeline,
//! prints it to stdout and sends it back to outbound direction in the pipeline. It's really important to add the
//! line delimiter because our pipeline will use a line decoder.
//! ```ignore
//! struct EchoServerHandler {
//!     transmits: VecDeque<TaggedString>,
//! }
//!
//! impl Handler for EchoServerHandler {
//!     type Rin = TaggedString;
//!     type Rout = Self::Rin;
//!     type Win = TaggedString;
//!     type Wout = Self::Win;
//!
//!     fn name(&self) -> &str {
//!         "EchoServerHandler"
//!     }
//!
//!     fn handle_read(
//!         &mut self,
//!         _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
//!         msg: Self::Rin,
//!     ) {
//!         println!("handling {}", msg.message);
//!         self.transmits.push_back(TaggedString {
//!             now: Instant::now(),
//!             transport: msg.transport,
//!             message: format!("{}\r\n", msg.message),
//!         });
//!     }
//!
//!     fn poll_write(
//!         &mut self,
//!         ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
//!     ) -> Option<Self::Wout> {
//!         if let Some(msg) = ctx.fire_poll_write() {
//!             self.transmits.push_back(msg);
//!         }
//!         self.transmits.pop_front()
//!     }
//! }
//! ```
//!
//! This needs to be the final handler in the pipeline. Now the definition of the pipeline is needed to handle the requests and responses.
//! ```ignore
//! let mut bootstrap = BootstrapServerTcp::new();
//! bootstrap.pipeline(Box::new(move || {
//!     let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
//!
//!     let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
//!         LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
//!     ));
//!     let string_codec_handler = TaggedStringCodec::new();
//!     let echo_server_handler = EchoServerHandler::new();
//!
//!     pipeline.add_back(line_based_frame_decoder_handler);
//!     pipeline.add_back(string_codec_handler);
//!     pipeline.add_back(echo_server_handler);
//!     pipeline.finalize()
//! }));
//! ```
//!
//! It is very important to be strict in the order of insertion as they are ordered by insertion. The pipeline has 4 handlers:
//!
//! * [TaggedByteToMessageCodec](crate::codec::byte_to_message_decoder::TaggedByteToMessageCodec)
//!     * Inbound: receives a zero-copy byte buffer and splits on line-endings
//!     * Outbound: just passes the byte buffer to AsyncTransport
//! * [TaggedStringCodec](crate::codec::string_codec::TaggedStringCodec)
//!     * Inbound: receives a byte buffer and decodes it into a std::string and pass up to the EchoHandler.
//!     * Outbound: receives a std::string and encodes it into a byte buffer and pass down to the TaggedByteToMessageCodec.
//! * EchoHandler
//!     * Inbound: receives a std::string and writes it to the pipeline, which will send the message outbound.
//!     * Outbound: receives a std::string and forwards it to TaggedStringCodec.
//!
//! Now that all needs to be done is plug the pipeline factory into a [BootstrapServerTcp](crate::bootstrap::BootstrapTcpServer) and that’s pretty much it.
//! Bind a local host:port and wait for it to stop.
//!
//! ```ignore
//! bootstrap.bind(format!("{}:{}", host, port)).await?;
//!
//! println!("Press ctrl-c to stop");
//! tokio::select! {
//!     _ = tokio::signal::ctrl_c() => {
//!         bootstrap.graceful_stop().await;
//!     }
//! };
//! ```
//!
//! ### Echo Client Example
//! The code for the echo client is very similar to the Echo Server. Here is the main echo handler.
//!
//! ```ignore
//! struct EchoClientHandler;
//!
//! impl Handler for EchoClientHandler {
//!     type Rin = TaggedString;
//!     type Rout = Self::Rin;
//!     type Win = TaggedString;
//!     type Wout = Self::Win;
//!
//!     fn name(&self) -> &str {
//!         "EchoClientHandler"
//!     }
//!
//!     fn handle_read(
//!         &mut self,
//!         _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
//!         msg: Self::Rin,
//!     ) {
//!         println!("received back: {}", msg.message);
//!     }
//!
//!     fn read_exception(
//!         &mut self,
//!         ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
//!         err: Box<dyn Error + Send + Sync>,
//!     ) {
//!         println!("received exception: {}", err);
//!         ctx.fire_close();
//!     }
//!
//!     fn read_eof(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
//!         println!("EOF received :(");
//!         ctx.fire_close();
//!     }
//!
//!     fn poll_write(
//!         &mut self,
//!         ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
//!     ) -> Option<Self::Wout> {
//!         ctx.fire_poll_write()
//!     }
//! }
//! ```
//!
//! Notice that we override other methods—read_exception and read_eof.
//! There are few other methods that can be overriden. If you need to handle a particular event,
//! just override the corresponding method.
//!
//! Now onto the client’s pipeline factory. It is identical the server’s pipeline factory, which
//! handles writing data.
//! ```ignore
//! let mut bootstrap = BootstrapClientTcp::new();
//! bootstrap.pipeline(Box::new( move || {
//!     let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
//!
//!     let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
//!         LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
//!     ));
//!     let string_codec_handler = TaggedStringCodec::new();
//!     let echo_client_handler = EchoClientHandler::new();
//!
//!     pipeline.add_back(line_based_frame_decoder_handler);
//!     pipeline.add_back(string_codec_handler);
//!     pipeline.add_back(echo_client_handler);
//!     pipeline.finalize()
//! }));
//! ```
//!
//! Now that all needs to be done is plug the pipeline factory into a BootstrapTcpClient and that’s pretty much it.
//! Connect to the remote peer and then read line from stdin and write it to pipeline.
//! ```ignore
//! let pipeline = bootstrap.connect(format!("{}:{}", host, port)).await?;
//!
//! println!("Enter bye to stop");
//! let mut buffer = String::new();
//! while tokio::io::stdin().read_line(&mut buffer).await.is_ok() {
//!     match buffer.trim_end() {
//!         "" => break,
//!         line => {
//!             pipeline.write(TaggedString {
//!                 now: Instant::now(),
//!                 transport,
//!                 message: format!("{}\r\n", line),
//!             });
//!             if line == "bye" {
//!                 pipeline.close();
//!                 break;
//!             }
//!         }
//!     };
//!     buffer.clear();
//! }
//!
//! bootstrap.graceful_stop().await;
//! ```
#![doc(html_logo_url = "https://raw.githubusercontent.com/retty-io/retty/master/docs/retty.io.jpg")]
#![warn(rust_2018_idioms)]
#![allow(dead_code)]
#![warn(missing_docs)]

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod executor;
pub mod transport;
