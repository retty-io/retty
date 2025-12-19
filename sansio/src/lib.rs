//! # SansIO - IO-Free Networking Framework
//!
//! `sansio` is a Sans-IO networking framework for Rust that separates protocol logic from I/O operations,
//! making it easy to build modular, reusable, and testable network protocols.
//!
//! Inspired by [Netty](https://netty.io) and [Wangle](https://github.com/facebook/wangle),
//! `sansio` brings the power of pipeline-based protocol composition to Rust.
//!
//! ## Core Concepts
//!
//! ### Pipeline Variants
//!
//! Sansio provides three pipeline variants to match different ownership and threading needs:
//!
//! - **[`Pipeline`]**: Exclusive ownership with `&mut self` methods. Zero overhead, ideal for
//!   per-connection TCP scenarios where one task owns the pipeline.
//!
//! - **[`RcPipeline`]**: Shared ownership with `Rc` wrapper and interior mutability. Methods take
//!   `&self`, enabling shared access in single-threaded contexts. Perfect for UDP servers where
//!   one pipeline handles messages from multiple peers.
//!
//! - **[`ArcPipeline`]**: Thread-safe shared ownership with `Arc` wrapper and `Mutex` synchronization.
//!   Methods take `&self` and can be called from multiple threads. Use when you need to share a
//!   pipeline across threads in multi-threaded servers.
//!
//! All pipelines are chains of [`Handler`]s that process inbound and outbound data, implementing
//! an advanced form of the Intercepting Filter pattern.
//!
//! **Key Benefits:**
//! - Modular: Each handler does one thing well (UNIX philosophy)
//! - Composable: Chain handlers to build complex protocols
//! - Flexible: Easy to add, remove, or reorder handlers
//! - Testable: Test handlers in isolation without I/O
//!
//! ### Handler
//!
//! A [`Handler`] processes messages flowing through the pipeline. Each handler has four associated types:
//! - `Rin`: Input type for inbound messages
//! - `Rout`: Output type for inbound messages
//! - `Win`: Input type for outbound messages
//! - `Wout`: Output type for outbound messages
//!
//! **Best Practice:** Keep handlers focused on a single responsibility. If a handler does multiple
//! things, split it into separate handlers.
//!
//! ### Protocol
//!
//! The [`Protocol`] trait provides a simpler alternative to [`Handler`] for building Sans-IO protocols.
//! It's fully decoupled from I/O, timers, and other runtime dependencies, making protocols easy to
//! test and reuse across different runtime environments.
//!
//! ## Choosing a Pipeline Variant
//!
//! | Feature | Pipeline | RcPipeline | ArcPipeline |
//! |---------|----------|------------|-------------|
//! | Ownership | Exclusive (`&mut self`) | Shared (`Rc`) | Shared (`Arc`) |
//! | Thread-safe | No | No | Yes |
//! | Overhead | None | `RefCell` + `UnsafeCell` | `Mutex` |
//! | Processing | Direct | Direct | Serialized |
//! | Best for | TCP (per-connection) | UDP (single-threaded) | Multi-threaded servers |
//!
//! **Use [`Pipeline`]** when:
//! - Each connection has its own exclusive pipeline (typical TCP server)
//! - You want zero-cost abstractions
//! - Single task/thread owns the pipeline
//!
//! **Use [`RcPipeline`]** when:
//! - Multiple contexts need to write to the same pipeline (UDP broadcast/multicast)
//! - Single-threaded executor (e.g., tokio `LocalSet`)
//! - Handler needs weak reference to its own pipeline
//!
//! **Use [`ArcPipeline`]** when:
//! - Pipeline must be shared across threads
//! - Multi-threaded server architecture
//! - Async runtime without `LocalSet` (thread pool)
//!
//! ## Event Flow
//!
//! Messages flow through the pipeline in two directions:
//! - **Inbound** (bottom-up): Network → Handler 1 → Handler 2 → ... → Handler N
//! - **Outbound** (top-down): Handler N → ... → Handler 2 → Handler 1 → Network
//!
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
//!   |          Context.fire_handle_read()               |               |
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
//!                   | handle_read()                     | poll_write()
//!                   |                                  \|/
//!   +---------------+-----------------------------------+---------------+
//!   |               |                                   |               |
//!   |        Internal I/O Threads (Transport Implementation)            |
//!   +-------------------------------------------------------------------+
//! ```
//!
//! ## Example: Echo Server
//!
//! Here's a complete example showing how to build a simple echo server using the pipeline pattern.
//!
//! ### Step 1: Define the Handler
//!
//! The echo handler receives strings, prints them, and sends them back:
//! ```ignore
//! struct EchoServerHandler {
//!     transmits: VecDeque<String>,
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
//!         self.transmits.push_back(format!("{}\r\n", msg.message));
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
//! ### Step 2: Build the Pipeline
//!
//! Chain handlers together to form a complete protocol stack:
//! ```ignore
//! // For TCP (exclusive ownership):
//! fn build_pipeline() -> Pipeline<BytesMut, String> {
//!     let mut pipeline = Pipeline::new();
//!
//!     let line_based_frame_decoder_handler = ByteToMessageCodecHandler::new(Box::new(
//!         LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
//!     ));
//!     let string_codec_handler = StringCodecHandler::new();
//!     let echo_server_handler = EchoServerHandler::new();
//!
//!     pipeline.add_back(line_based_frame_decoder_handler);
//!     pipeline.add_back(string_codec_handler);
//!     pipeline.add_back(echo_server_handler);
//!     pipeline.finalize();
//!
//!     pipeline
//! }
//!
//! // For UDP (shared ownership):
//! fn build_shared_pipeline() -> Rc<RcPipeline<BytesMut, String>> {
//!     let mut pipeline = RcPipeline::new();
//!     // ... add handlers ...
//!     pipeline.finalize();
//!     Rc::new(pipeline)
//! }
//! ```
//!
//! **Handler Responsibilities:**
//! - **LineBasedFrameDecoder**: Splits byte stream on `\n` or `\r\n`
//! - **StringCodec**: Converts bytes ↔ UTF-8 strings
//! - **EchoHandler**: Application logic (echo messages back)
//!
//! **Important:** Handler order matters! They're processed in insertion order.
//!
//! ### Step 3.1: Run the Event Loop with sync fn
//!
//! The pipeline is pure protocol logic - you provide the I/O:
//! ```ignore
//! fn run(socket: UdpSocket, cancel_rx: crossbeam_channel::Receiver<()>) {
//!     let mut buf = vec![0; 2000];
//!
//!     let pipeline = build_pipeline();
//!     pipeline.transport_active();
//!     loop {
//!         // Check cancellation
//!         if cancel_rx.try_recv().is_ok() {
//!             break;
//!         }
//!
//!         // Poll pipeline to write transmit to socket
//!         while let Some(transmit) = pipeline.poll_write() {
//!             socket.send(transmit)?;
//!         }
//!
//!         // Poll pipeline to get next timeout
//!         let mut eto = Instant::now() + Duration::from_millis(100);
//!         pipeline.poll_timeout(&mut eto);
//!
//!         let delay_from_now = eto
//!             .checked_duration_since(Instant::now())
//!             .unwrap_or(Duration::from_secs(0));
//!         if delay_from_now.is_zero() {
//!            pipeline.handle_timeout(Instant::now());
//!            continue;
//!         }
//!
//!         socket.set_read_timeout(Some(delay_from_now)).expect("setting socket read timeout");
//!         if let Ok(n) = socket.recv_from(buf) {
//!             pipeline.handle_read(&buf[..n]);
//!         }
//!
//!         // Drive time forward
//!         pipeline.handle_timeout(Instant::now());
//!     }
//!     pipeline.transport_inactive();
//! }
//! ```
//! ### Step 3.2: Run the Event Loop with async fn
//!
//! The pipeline is pure protocol logic - you provide the I/O:
//! ```ignore
//! async fn run(socket: UdpSocket,
//!              mut close_rx: broadcast::Receiver<()>,
//!              mut write_notify: Notify) {
//!     let mut buf = vec![0; 2000];
//!
//!     let pipeline = build_pipeline();
//!     pipeline.transport_active();
//!     loop {
//!         // Poll pipeline to write transmit to socket
//!         while let Some(transmit) = pipeline.poll_write() {
//!             socket.send(transmit)?;
//!         }
//!
//!         // Poll pipeline to get next timeout
//!         let mut eto = Instant::now() + Duration::from_secs(86400);
//!         pipeline.poll_timeout(&mut eto);
//!
//!         let delay_from_now = eto
//!             .checked_duration_since(Instant::now())
//!             .unwrap_or(Duration::from_secs(0));
//!         if delay_from_now.is_zero() {
//!            pipeline.handle_timeout(Instant::now());
//!            continue;
//!         }
//!
//!         let timer = tokio::time::sleep(delay_from_now);
//!         tokio::pin!(timer);
//!
//!         tokio::select! {
//!             _ = close_rx.recv() => break,
//!             _ = write_notify.notified() => {},
//!             _ = timer.as_mut() => pipeline.handle_timeout(Instant::now()),
//!             n = socket.recv(&mut buf) => pipeline.handle_read(&buf[..n]),
//!         }
//!     }
//!     pipeline.transport_inactive();
//! }
//! ```
//!
//! ## Design Philosophy
//!
//! `sansio` follows the Sans-IO pattern, which separates protocol logic from I/O concerns:
//!
//! **Benefits:**
//! - **Testable**: Test protocol logic without real network I/O
//! - **Flexible**: Use with any I/O model (sync, async, embedded)
//! - **Reusable**: Same protocol code across different environments
//! - **Debuggable**: Easier to reason about and debug
//!
//! **Trade-offs:**
//! - You manage the I/O loop yourself
//! - More control means more responsibility
//! - Steeper learning curve for simple cases
//!
//! For most applications, the benefits far outweigh the trade-offs, especially as your
//! protocol logic becomes more complex.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/webrtc-rs/sansio/master/doc/sansio-white.png"
)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)]
#![warn(missing_docs)]

// ========================================
// Module Declarations
// ========================================

/// Handler implementations for protocol pipelines
pub(crate) mod handler;

/// Internal handler types (not part of public API)
pub(crate) mod handler_internal;

/// Pipeline implementations for chaining handlers
pub(crate) mod pipeline;

/// Internal pipeline types (not part of public API)
pub(crate) mod pipeline_internal;

/// Internal Rc-based pipeline types with interior mutability
pub(crate) mod rc_pipeline_internal;

/// Rc-based pipeline with interior mutability for shared ownership
pub(crate) mod rc_pipeline;

/// Internal Arc-based pipeline types with Mutex synchronization
pub(crate) mod arc_pipeline_internal;

/// Arc-based pipeline for multi-threaded usage
pub(crate) mod arc_pipeline;

/// Protocol trait for building Sans-IO protocols
pub(crate) mod protocol;

// ========================================
// Public Exports
// ========================================

/// Handler and context types for building protocol pipelines
pub use handler::{Context, Handler};

/// Pipeline traits for inbound and outbound message processing
pub use pipeline::{InboundPipeline, OutboundPipeline, Pipeline};

/// Rc-based pipeline with interior mutability
pub use rc_pipeline::{RcInboundPipeline, RcOutboundPipeline, RcPipeline};

/// Arc-based pipeline for multi-threaded usage
pub use arc_pipeline::{ArcInboundPipeline, ArcOutboundPipeline, ArcPipeline};

/// Protocol trait for Sans-IO protocol implementations
pub use protocol::Protocol;

/// Callback type for write notifications across all pipeline variants
pub type NotifyCallback = std::sync::Arc<dyn Fn() + Send + Sync>;

/// Reserved handler name used internally by the pipeline's LastHandler
pub(crate) const RESERVED_PIPELINE_HANDLE_NAME: &str = "ReservedPipelineHandlerName";
