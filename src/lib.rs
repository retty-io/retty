//! ### What is Retty?
//! Retty is an asynchronous Rust networking framework that makes it easy to build protocols, application clients/servers.
//!
//! It's like [Netty](https://netty.io) or [Wangle](https://github.com/facebook/wangle), but in Rust.
//!
//! ### What is a Pipeline?
//! The fundamental abstraction of Retty is the Pipeline. Once you have fully understood this abstraction,
//! you will be able to write all sorts of sophisticated protocols, application clients/servers.
//! The pipeline is the most important and powerful abstraction of Retty. It offers immense flexibility
//! to customize how requests and responses are handled by your service.
//!  
//! A pipeline is a chain of request/response handlers that handle upstream (handling request) and
//! downstream (handling response). Once you chain handlers together, it provides an agile way to convert
//! a raw data stream into the desired message type (class) and the inverse -- desired message type to raw data stream.
//!
//! A handler should do one and only one function - just like the UNIX philosophy. If you have a handler that
//! is doing more than one function than you should split it into individual handlers. This is really important for
//! maintainability and flexibility as its common to change your protocol for one reason or the other.
//! All shared state within handlers are not thread-safe. Only use shared state that is guarded by a mutex, atomic lock, etc.
//!
//! A list of Handlers which handles or intercepts inbound events and outbound operations.
//! Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
//! over how an event is handled and how the Handlers in a pipeline interact with each other.
//!
//! ### How an event flows in a pipeline
//! ```text
//!                                                  Write Request
//!                                                       |
//!   +---------------------------------------------------+---------------+
//!   |                             Pipeline              |               |
//!   |                                                  \|/              |
//!   |    +---------------------+            +-----------+----------+    |
//!   |    | Inbound Handler  N  | ---------> | Outbound Handler  1  |    |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |              /|\                                  |               |
//!   |               |                                  \|/              |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |    | Inbound Handler N-1 | ---------> | Outbound Handler  2  |    |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |              /|\                                  .               |
//!   |               .                                   .               |
//!   | Inbound HandlerContext.fire_*()  Outbound HandlerContext.fire_*() |
//!   |        [ method call]                       [method call]         |
//!   |               .                                   .               |
//!   |               .                                  \|/              |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |    | Inbound Handler  2  | ---------> | Outbound Handler M-1 |    |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |              /|\                                  |               |
//!   |               |                                  \|/              |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |    | Inbound Handler  1  | ---------> | Outbound Handler  M  |    |
//!   |    +----------+----------+            +-----------+----------+    |
//!   |              /|\                                  |               |
//!   +---------------+-----------------------------------+---------------+
//!                   |                                  \|/
//!   +---------------+-----------------------------------+---------------+
//!   |               |                                   |               |
//!   |  [ AsyncTransport.read() ]            [ AsyncTransport.write() ]  |
//!   |                                                                   |
//!   |        Internal I/O Threads (Transport Implementation)            |
//!   +-------------------------------------------------------------------+
//! ```
//!
//! ### Echo Server Example
//! Let's look at how to write an echo server.
//!
//! Here's the main piece of code in our echo server; it receives a string, prints it to stdout and
//! sends it back downstream in the pipeline. It's really important to add the line delimiter because
//! our pipeline will use a line decoder.
//! ```ignore
//! struct EchoHandler {
//!     decoder: EchoDecoder,
//!     encoder: EchoEncoder,
//! }
//!
//! #[async_trait]
//! impl InboundHandler for EchoDecoder {
//!     type Rin = String;
//!     type Rout = Self::Rin;
//!
//!     async fn read(&mut self,
//!                   ctx: &mut InboundHandlerContext<Self::Rout>,
//!                   message: &mut Self::Rin) {
//!         println!("handling {}", message);
//!         ctx.fire_write(&mut format!("{}\r\n", message)).await;
//!     }
//! }
//!
//! #[async_trait]
//! impl OutboundHandler for EchoEncoder {
//!     type Win = String;
//!     type Wout = Self::Win;
//!
//!     async fn write(&mut self,
//!                    ctx: &mut OutboundHandlerContext<Self::Wout>,
//!                    message: &mut Self::Win) {
//!         ctx.fire_write(message).await;
//!     }
//! }
//!
//! impl Handler for EchoHandler {
//!     type In = String;
//!     type Out = Self::In;
//!
//!     fn name(&self) -> &str {
//!         "EchoHandler"
//!     }
//!
//!     fn split(
//!         self,
//!     ) -> (
//!         Arc<Mutex<dyn InboundHandlerInternal>>,
//!         Arc<Mutex<dyn OutboundHandlerInternal>>,
//!     ) {
//!         let inbound_handler: Box<dyn InboundHandler<Rin = Self::In, Rout = Self::Out>> =
//!             Box::new(self.decoder);
//!         let outbound_handler: Box<dyn OutboundHandler<Win = Self::Out, Wout = Self::In>> =
//!             Box::new(self.encoder);
//!
//!         (
//!             Arc::new(Mutex::new(inbound_handler)),
//!             Arc::new(Mutex::new(outbound_handler)),
//!         )
//!     }
//! }
//! ```
//!
//! This needs to be the final handler in the pipeline. Now the definition of the pipeline is needed to handle the requests and responses.
//! ```ignore
//! let mut bootstrap = BootstrapTcpServer::new(default_runtime().unwrap());
//!     bootstrap
//!         .pipeline(Box::new(
//!             move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
//!                 let mut pipeline = Pipeline::new();
//!
//!                 let async_transport_handler = AsyncTransportTcp::new(sock);
//!                 let line_based_frame_decoder_handler = ByteToMessageCodec::new(Box::new(
//!                     LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
//!                 ));
//!                 let string_codec_handler = StringCodec::new();
//!                 let echo_handler = EchoHandler::new();
//!
//!                 pipeline.add_back(async_transport_handler);
//!                 pipeline.add_back(line_based_frame_decoder_handler);
//!                 pipeline.add_back(string_codec_handler);
//!                 pipeline.add_back(echo_handler);
//!
//!                 Box::pin(async move { pipeline.finalize().await })
//!             },
//!         ))
//!         .bind(format!("{}:{}", host, port))
//!         .await?;
//! ```
//!
//! It is very important to be strict in the order of insertion as they are ordered by insertion. The pipeline has 4 handlers:
//!
//! * AsyncTransportTcp
//!     * Inbound: Reads a raw data stream from the socket and converts it into a zero-copy byte buffer.
//!     * Outbound: Writes the contents of a zero-copy byte buffer to the underlying socket.
//! * ByteToMessageCodec
//!     * Inbound: receives a zero-copy byte buffer and splits on line-endings
//!     * Outbound: just passes the byte buffer to AsyncTransportTcp
//! * StringCodec
//!     * Inbound: receives a byte buffer and decodes it into a std::string and pass up to the EchoHandler.
//!     * Outbound: receives a std::string and encodes it into a byte buffer and pass down to the ByteToMessageCodec.
//! * EchoHandler
//!     * Inbound: receives a std::string and writes it to the pipeline, which will send the message outbound.
//!     * Outbound: receives a std::string and forwards it to StringCodec.
//!
//! Now that all needs to be done is plug the pipeline factory into a BootstrapTcpServer and that’s pretty much it.
//! Bind a port and wait for it to stop.
//!
//! ```ignore
//! tokio::select! {
//!        _ = tokio::signal::ctrl_c() => {
//!            bootstrap.stop().await;
//!        }
//!    };
//! ```

#![warn(rust_2018_idioms)]
#![allow(dead_code)]
#![warn(missing_docs)]

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod error;
pub mod runtime;
pub mod transport;
