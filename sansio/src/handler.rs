//! # Handler Trait - Pipeline-Based Protocol Processing
//!
//! The [`Handler`] trait is the core building block for constructing protocol pipelines in sansio.
//! Handlers process messages flowing through a pipeline, transforming them from one type to another
//! while maintaining strict type safety.
//!
//! ## Overview
//!
//! A handler is a bidirectional message processor that sits in a pipeline:
//! - **Inbound direction**: Receives `Rin`, processes it, produces `Rout` for the next handler
//! - **Outbound direction**: Receives `Win`, processes it, produces `Wout` for the previous handler
//!
//! Each handler has a [`Context`] that allows it to:
//! - Forward messages to the next handler in the pipeline
//! - Poll messages from the next handler
//! - Propagate events (timeouts, errors, EOF, etc.)
//!
//! ## Type Parameters
//!
//! Handlers have four associated types that define the transformation:
//!
//! - `Rin`: **R**ead **in**put - What this handler receives from the previous handler (inbound)
//! - `Rout`: **R**ead **out**put - What this handler produces for the next handler (inbound)
//! - `Win`: **W**rite **in**put - What this handler receives from the next handler (outbound)
//! - `Wout`: **W**rite **out**put - What this handler produces for the previous handler (outbound)
//!
//! ## Handler Chain Type Flow
//!
//! ```text
//! Pipeline: Handler1 -> Handler2 -> Handler3
//!
//! Inbound (handle_read):
//!   Handler1: Rin1 -> Rout1
//!   Handler2: Rin2 (= Rout1) -> Rout2
//!   Handler3: Rin3 (= Rout2) -> Rout3
//!
//! Outbound (poll_write):
//!   Handler3: Win3 -> Wout3
//!   Handler2: Win2 (= Wout3) -> Wout2
//!   Handler1: Win1 (= Wout2) -> Wout1
//! ```
//!
//! ## Example: Echo Handler
//!
//! ```rust
//! use sansio::{Handler, Context};
//! use std::collections::VecDeque;
//!
//! /// Echo handler that receives strings and queues them to be sent back
//! struct EchoHandler {
//!     output_queue: VecDeque<String>,
//! }
//!
//! impl EchoHandler {
//!     fn new() -> Self {
//!         Self {
//!             output_queue: VecDeque::new(),
//!         }
//!     }
//! }
//!
//! impl Handler for EchoHandler {
//!     // Input type: String
//!     type Rin = String;
//!     // Output type: Same as input (passthrough for inbound)
//!     type Rout = String;
//!     // Write input type: String
//!     type Win = String;
//!     // Write output type: String to send back
//!     type Wout = String;
//!
//!     fn name(&self) -> &str {
//!         "EchoHandler"
//!     }
//!
//!     fn handle_read(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, msg: Self::Rin) {
//!         println!("Echoing: {}", msg);
//!         // Queue the message to be sent back
//!         self.output_queue.push_back(msg.clone());
//!         // Also forward to next handler (if any)
//!         ctx.fire_handle_read(msg);
//!     }
//!
//!     fn poll_write(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> {
//!         // First check if next handler has data
//!         if let Some(msg) = ctx.fire_poll_write() {
//!             return Some(msg);
//!         }
//!         // Then return our queued data
//!         self.output_queue.pop_front()
//!     }
//! }
//! ```
//!
//! ## Example: Protocol Codec Handler
//!
//! ```rust
//! use sansio::{Handler, Context};
//! use std::collections::VecDeque;
//!
//! # #[derive(Debug, Clone)]
//! # struct Message { data: String }
//!
//! /// Decodes bytes into messages, encodes messages into bytes
//! struct ProtocolCodec {
//!     read_buffer: Vec<u8>,
//!     write_queue: VecDeque<Vec<u8>>,
//! }
//!
//! impl Handler for ProtocolCodec {
//!     // Inbound: receive bytes, produce Messages
//!     type Rin = Vec<u8>;
//!     type Rout = Message;
//!     // Outbound: receive Messages, produce bytes
//!     type Win = Message;
//!     type Wout = Vec<u8>;
//!
//!     fn name(&self) -> &str {
//!         "ProtocolCodec"
//!     }
//!
//!     fn handle_read(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, msg: Self::Rin) {
//!         // Decode bytes into message
//!         self.read_buffer.extend_from_slice(&msg);
//!
//!         // Simple example: assume we have a complete message
//!         if let Ok(s) = String::from_utf8(self.read_buffer.clone()) {
//!             let message = Message { data: s };
//!             self.read_buffer.clear();
//!             // Forward decoded message to next handler
//!             ctx.fire_handle_read(message);
//!         }
//!     }
//!
//!     fn poll_write(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> {
//!         // Poll for messages from next handler
//!         if let Some(msg) = ctx.fire_poll_write() {
//!             // Encode message to bytes
//!             self.write_queue.push_back(msg.data.into_bytes());
//!         }
//!         // Return encoded bytes
//!         self.write_queue.pop_front()
//!     }
//! }
//! ```
//!
//! ## Event Handling
//!
//! Handlers can also process lifecycle and error events:
//!
//! - **`transport_active`**: Connection established
//! - **`transport_inactive`**: Connection closed
//! - **`handle_timeout`**: Periodic timeout for time-based operations
//! - **`handle_error`**: Error occurred in the pipeline
//! - **`handle_eof`**: End-of-file/stream reached
//! - **`handle_close`**: Explicit close request
//!
//! ## Context API
//!
//! The [`Context`] object provides methods to interact with the pipeline:
//!
//! - **`fire_handle_read(msg)`**: Forward a processed inbound message
//! - **`fire_poll_write()`**: Poll for outbound messages from next handler
//! - **`fire_transport_active()`**: Propagate transport active event
//! - **`fire_handle_timeout(now)`**: Propagate timeout event
//! - **`fire_handle_error(err)`**: Propagate error event
//!
//! ## Best Practices
//!
//! 1. **Single Responsibility**: Each handler should do one thing well
//! 2. **Type Transformation**: Use type parameters to document transformations
//! 3. **Event Propagation**: Always call `ctx.fire_*` to propagate events unless you explicitly want to stop them
//! 4. **Error Handling**: Convert errors into appropriate types or propagate via `fire_handle_error`
//! 5. **Buffering**: Use internal queues when transformations are one-to-many or many-to-one
//!
//! ## Comparison with Protocol Trait
//!
//! | Feature | Handler | Protocol |
//! |---------|---------|----------|
//! | Complexity | More complex | Simple |
//! | Context | Has context object | No context |
//! | Composition | Pipeline-based | Manual |
//! | Best for | Protocol stacks | Single protocols |
//!
//! Use `Handler` when building complex protocol pipelines with multiple layers.
//! Use [`Protocol`](crate::Protocol) when you want a simple, self-contained protocol.

use crate::handler_internal::{ContextInternal, HandlerInternal};
use log::warn;
use std::any::Any;
use std::marker::PhantomData;
use std::{error::Error, time::Instant};

/// A handler processes messages flowing through a pipeline.
///
/// The `Handler` trait defines bidirectional message processing with strict type safety.
/// Each handler transforms messages from one type to another, enabling complex protocol
/// stacks to be built from simple, composable components.
///
/// # Type Parameters
///
/// - `Rin`: Read input - what this handler receives (inbound direction)
/// - `Rout`: Read output - what this handler produces (inbound direction)
/// - `Win`: Write input - what this handler receives (outbound direction)
/// - `Wout`: Write output - what this handler produces (outbound direction)
///
/// # Design Pattern
///
/// Handlers follow the Intercepting Filter pattern:
/// 1. Receive messages from one direction
/// 2. Transform the message (decode, encode, validate, etc.)
/// 3. Forward to the next handler in the pipeline
/// 4. Optionally handle events (timeouts, errors, lifecycle)
///
/// # Example
///
/// See the [module-level documentation](index.html) for complete examples.
pub trait Handler {
    /// Read input message type.
    ///
    /// This is the type of messages this handler receives from the previous handler
    /// in the pipeline (inbound direction).
    type Rin: 'static;

    /// Read output message type.
    ///
    /// This is the type of messages this handler produces for the next handler
    /// in the pipeline (inbound direction).
    type Rout: 'static;

    /// Write input message type.
    ///
    /// This is the type of messages this handler receives from the next handler
    /// in the pipeline (outbound direction).
    type Win: 'static;

    /// Write output message type.
    ///
    /// This is the type of messages this handler produces for the previous handler
    /// in the pipeline (outbound direction).
    type Wout: 'static;

    /// Returns the handler's name.
    ///
    /// Used for debugging, logging, and error messages. Should be unique
    /// within a pipeline to help identify which handler is being referenced.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::Handler;
    /// # struct MyHandler;
    /// # impl Handler for MyHandler {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = String;
    /// fn name(&self) -> &str {
    ///     "MyProtocolHandler"
    /// }
    /// #     fn handle_read(&mut self, _ctx: &sansio::Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, _msg: Self::Rin) {}
    /// #     fn poll_write(&mut self, _ctx: &sansio::Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> { None }
    /// # }
    /// ```
    fn name(&self) -> &str;

    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    fn generate(self) -> (String, Box<dyn HandlerInternal>, Box<dyn ContextInternal>)
    where
        Self: Sized + 'static,
    {
        let handler_name = self.name().to_owned();
        let context: Context<Self::Rin, Self::Rout, Self::Win, Self::Wout> =
            Context::new(self.name());

        let handler: Box<
            dyn Handler<Rin = Self::Rin, Rout = Self::Rout, Win = Self::Win, Wout = Self::Wout>,
        > = Box::new(self);

        (handler_name, Box::new(handler), Box::new(context))
    }

    /// Called when the transport becomes active (connected).
    ///
    /// This event is triggered when a connection is established. Handlers can use this
    /// to initialize state, start timers, or perform handshakes.
    ///
    /// The default implementation propagates the event to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{Handler, Context};
    /// # struct MyHandler { connected: bool }
    /// # impl Handler for MyHandler {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = String;
    /// #     fn name(&self) -> &str { "MyHandler" }
    /// fn transport_active(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
    ///     self.connected = true;
    ///     println!("Connection established!");
    ///     // Propagate to next handler
    ///     ctx.fire_transport_active();
    /// }
    /// #     fn handle_read(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, _msg: Self::Rin) {}
    /// #     fn poll_write(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> { None }
    /// # }
    /// ```
    fn transport_active(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_transport_active();
    }

    /// Called when the transport becomes inactive (disconnected).
    ///
    /// This event is triggered when a connection is closed. Handlers can use this
    /// to clean up resources, cancel timers, or save state.
    ///
    /// The default implementation propagates the event to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    fn transport_inactive(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_transport_inactive();
    }

    /// Handles an inbound message.
    ///
    /// This is the primary method for processing incoming data. The handler receives
    /// a message of type `Rin`, processes it, and can forward transformed messages
    /// of type `Rout` to the next handler via `ctx.fire_handle_read(msg)`.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    /// - `msg`: The incoming message to process
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{Handler, Context};
    /// # struct ToUpperHandler;
    /// # impl Handler for ToUpperHandler {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = String;
    /// #     fn name(&self) -> &str { "ToUpperHandler" }
    /// fn handle_read(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, msg: Self::Rin) {
    ///     // Transform the message
    ///     let transformed = msg.to_uppercase();
    ///     // Forward to next handler
    ///     ctx.fire_handle_read(transformed);
    /// }
    /// #     fn poll_write(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> { None }
    /// # }
    /// ```
    fn handle_read(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    );

    /// Polls for an outbound message.
    ///
    /// This method is called to retrieve messages that should be written to the transport.
    /// Handlers typically:
    /// 1. Poll the next handler via `ctx.fire_poll_write()`
    /// 2. Transform received messages or generate their own
    /// 3. Return messages of type `Wout`
    ///
    /// Returns `None` when no message is available.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    ///
    /// # Returns
    ///
    /// - `Some(Wout)`: A message ready to be written
    /// - `None`: No message available
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{Handler, Context};
    /// # use std::collections::VecDeque;
    /// # struct BufferingHandler { queue: VecDeque<String> }
    /// # impl Handler for BufferingHandler {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = String;
    /// #     fn name(&self) -> &str { "BufferingHandler" }
    /// fn poll_write(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> {
    ///     // First check next handler
    ///     if let Some(msg) = ctx.fire_poll_write() {
    ///         return Some(msg);
    ///     }
    ///     // Then check our own queue
    ///     self.queue.pop_front()
    /// }
    /// #     fn handle_read(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, _msg: Self::Rin) {}
    /// # }
    /// ```
    fn poll_write(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout>;

    /// Handles a timeout event.
    ///
    /// Called periodically to allow handlers to perform time-based operations like:
    /// - Sending keepalive messages
    /// - Timing out pending operations
    /// - Retransmitting lost packets
    ///
    /// The default implementation propagates the event to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    /// - `now`: The current timestamp
    fn handle_timeout(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        now: Instant,
    ) {
        ctx.fire_handle_timeout(now);
    }

    /// Polls for the next timeout deadline.
    ///
    /// Handlers can update `eto` (earliest timeout) to indicate when they next
    /// need `handle_timeout` to be called. The pipeline will call the handler
    /// with the minimum timeout across all handlers.
    ///
    /// The default implementation propagates the poll to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    /// - `eto`: Mutable reference to the earliest timeout. Update this to request an earlier timeout.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{Handler, Context};
    /// # use std::time::{Instant, Duration};
    /// # struct KeepaliveHandler { next_keepalive: Option<Instant> }
    /// # impl Handler for KeepaliveHandler {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = String;
    /// #     fn name(&self) -> &str { "KeepaliveHandler" }
    /// fn poll_timeout(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, eto: &mut Instant) {
    ///     // Request timeout for our keepalive
    ///     if let Some(deadline) = self.next_keepalive {
    ///         if deadline < *eto {
    ///             *eto = deadline;
    ///         }
    ///     }
    ///     // Also check next handler
    ///     ctx.fire_poll_timeout(eto);
    /// }
    /// #     fn handle_read(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, _msg: Self::Rin) {}
    /// #     fn poll_write(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> { None }
    /// # }
    /// ```
    fn poll_timeout(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        eto: &mut Instant,
    ) {
        ctx.fire_poll_timeout(eto);
    }

    /// Handles an end-of-file (EOF) event.
    ///
    /// Called when the input stream reaches EOF (e.g., remote peer closed their write side).
    /// Handlers can use this to flush buffers or initiate graceful shutdown.
    ///
    /// The default implementation propagates the event to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    fn handle_eof(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_handle_eof();
    }

    /// Handles an error event.
    ///
    /// Called when an error occurs in the pipeline. Handlers can:
    /// - Log the error
    /// - Attempt recovery
    /// - Transform the error
    /// - Propagate it to the next handler
    ///
    /// The default implementation propagates the error to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    /// - `err`: The error that occurred
    fn handle_error(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        err: Box<dyn Error>,
    ) {
        ctx.fire_handle_error(err);
    }

    /// Handles a close event.
    ///
    /// Called when the pipeline is being closed. Handlers should clean up resources,
    /// flush buffers, and prepare for shutdown.
    ///
    /// The default implementation propagates the event to the next handler.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Context for interacting with the pipeline
    fn handle_close(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_handle_close();
    }
}

impl<Rin: 'static, Rout: 'static, Win: 'static, Wout: 'static> HandlerInternal
    for Box<dyn Handler<Rin = Rin, Rout = Rout, Win = Win, Wout = Wout>>
{
    fn transport_active_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.transport_active(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn transport_inactive_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.transport_inactive(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn handle_read_internal(&mut self, ctx: &dyn ContextInternal, msg: Box<dyn Any>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            if let Ok(msg) = msg.downcast::<Rin>() {
                self.handle_read(ctx, *msg);
            } else {
                panic!("msg can't downcast::<Rin> in {} handler", ctx.name());
            }
        } else {
            panic!(
                "ctx can't downcast::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn poll_write_internal(&mut self, ctx: &dyn ContextInternal) -> Option<Box<dyn Any>> {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            if let Some(msg) = self.poll_write(ctx) {
                Some(Box::new(msg))
            } else {
                None
            }
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn handle_timeout_internal(&mut self, ctx: &dyn ContextInternal, now: Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_timeout(ctx, now);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn poll_timeout_internal(&mut self, ctx: &dyn ContextInternal, eto: &mut Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.poll_timeout(ctx, eto);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn handle_eof_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_eof(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn handle_error_internal(&mut self, ctx: &dyn ContextInternal, err: Box<dyn Error>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_error(ctx, err);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn handle_close_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_close(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Enables a [`Handler`] to interact with the pipeline.
///
/// The `Context` object is passed to each handler method and provides the API
/// for forwarding messages, polling data, and propagating events through the
/// pipeline.
///
/// # Type Parameters
///
/// - `Rin`: Read input message type (what the current handler receives)
/// - `Rout`: Read output message type (what the current handler produces)
/// - `Win`: Write input message type (what the current handler receives for writing)
/// - `Wout`: Write output message type (what the current handler produces for writing)
///
/// # Architecture
///
/// Context uses raw pointers to link handlers in the chain:
///
/// ```text
/// Handler[0] ──> Handler[1] ──> Handler[2] ──> LastHandler
///    │              │              │               │
///    └─Context[0]   └─Context[1]   └─Context[2]   └─Context[3]
///       │              │              │
///       └──────────────┴──────────────┘
///          raw pointers to next handler/context
/// ```
///
/// Each context holds:
/// - `next_handler: *mut dyn HandlerInternal` - Pointer to the next handler
/// - `next_context: *mut dyn ContextInternal` - Pointer to the next context
///
/// These pointers are set during pipeline finalization and remain valid because:
/// - The pipeline owns all handlers and contexts for their lifetime
/// - Box ensures stable addresses even if the Vec reallocates
/// - Single-threaded use only (not thread-safe)
///
/// # Responsibilities
///
/// The context provides two primary functions:
///
/// 1. **Forward Operations**: Push data/events down the pipeline
///    - `fire_handle_read()`: Forward processed inbound messages
///    - `fire_transport_active()`: Propagate connection established event
///    - `fire_handle_timeout()`: Propagate timeout events
///
/// 2. **Pull Operations**: Pull data from the next handler
///    - `fire_poll_write()`: Poll outbound messages from next handler
///    - `fire_poll_timeout()`: Poll timeout requirements from next handler
///
/// # Example
///
/// ```rust
/// use sansio::{Handler, Context};
///
/// struct MyHandler;
///
/// impl Handler for MyHandler {
///     type Rin = String;
///     type Rout = String;
///     type Win = String;
///     type Wout = String;
///
///     fn name(&self) -> &str {
///         "MyHandler"
///     }
///
///     fn handle_read(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, msg: Self::Rin) {
///         // Use context to forward message
///         ctx.fire_handle_read(msg);
///     }
///
///     fn poll_write(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> {
///         // Use context to poll next handler
///         ctx.fire_poll_write()
///     }
/// }
/// ```
pub struct Context<Rin, Rout, Win, Wout> {
    /// Handler name for debugging and error messages
    name: String,

    /// Raw pointer to the next context in the handler chain.
    ///
    /// Set during pipeline finalization. None if this is the last handler.
    next_context: Option<*mut dyn ContextInternal>,

    /// Raw pointer to the next handler in the handler chain.
    ///
    /// Set during pipeline finalization. None if this is the last handler.
    next_handler: Option<*mut dyn HandlerInternal>,

    phantom: PhantomData<(Rin, Rout, Win, Wout)>,
}

impl<Rin: 'static, Rout: 'static, Win: 'static, Wout: 'static> Context<Rin, Rout, Win, Wout> {
    /// Creates a new Context with null handler/context pointers.
    ///
    /// The pointers will be set during pipeline finalization when the handler
    /// chain is established.
    ///
    /// Typically called internally by the pipeline when adding a handler.
    /// You rarely need to create a Context manually.
    ///
    /// # Parameters
    ///
    /// - `name`: The name of the handler this context belongs to
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            next_context: None,
            next_handler: None,
            phantom: PhantomData,
        }
    }

    /// Propagates transport active event to the next handler.
    ///
    /// Call this to forward the "connection established" event through the pipeline.
    /// This should be called from your [`Handler::transport_active`] implementation
    /// unless you explicitly want to stop event propagation.
    pub fn fire_transport_active(&self) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.transport_active_internal(context);
            }
        }
    }

    /// Propagates transport inactive event to the next handler.
    ///
    /// Call this to forward the "connection closed" event through the pipeline.
    /// This should be called from your [`Handler::transport_inactive`] implementation
    /// unless you explicitly want to stop event propagation.
    pub fn fire_transport_inactive(&self) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.transport_inactive_internal(context);
            }
        }
    }

    /// Forwards an inbound message to the next handler.
    ///
    /// After processing a message in [`Handler::handle_read`], call this method
    /// to pass the transformed message (`Rout`) to the next handler in the pipeline.
    ///
    /// # Parameters
    ///
    /// - `msg`: The processed message to forward
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{Handler, Context};
    /// # struct DecoderHandler;
    /// # impl Handler for DecoderHandler {
    /// #     type Rin = Vec<u8>;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = Vec<u8>;
    /// #     fn name(&self) -> &str { "DecoderHandler" }
    /// fn handle_read(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, msg: Self::Rin) {
    ///     // Decode bytes to string
    ///     if let Ok(decoded) = String::from_utf8(msg) {
    ///         // Forward decoded message
    ///         ctx.fire_handle_read(decoded);
    ///     }
    /// }
    /// #     fn poll_write(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> { None }
    /// # }
    /// ```
    pub fn fire_handle_read(&self, msg: Rout) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.handle_read_internal(context, Box::new(msg));
            }
        } else {
            warn!("handle_read reached end of pipeline");
        }
    }

    /// Polls the next handler for an outbound message.
    ///
    /// Call this from [`Handler::poll_write`] to retrieve messages from the next
    /// handler in the pipeline. Returns `Some(Win)` if the next handler has data
    /// to send, or `None` otherwise.
    ///
    /// # Returns
    ///
    /// - `Some(Win)`: A message from the next handler
    /// - `None`: No message available
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{Handler, Context};
    /// # use std::collections::VecDeque;
    /// # struct EncoderHandler { queue: VecDeque<Vec<u8>> }
    /// # impl Handler for EncoderHandler {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = Vec<u8>;
    /// #     fn name(&self) -> &str { "EncoderHandler" }
    /// fn poll_write(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> {
    ///     // Poll next handler first
    ///     if let Some(msg) = ctx.fire_poll_write() {
    ///         // Encode string to bytes
    ///         return Some(msg.into_bytes());
    ///     }
    ///     // Then return our own queued data
    ///     self.queue.pop_front()
    /// }
    /// #     fn handle_read(&mut self, _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, _msg: Self::Rin) {}
    /// # }
    /// ```
    pub fn fire_poll_write(&self) -> Option<Win> {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                if let Some(msg) = handler.poll_write_internal(context) {
                    if let Ok(msg) = msg.downcast::<Win>() {
                        Some(*msg)
                    } else {
                        panic!("msg can't downcast::<Win> in {} handler", context.name());
                    }
                } else {
                    None
                }
            }
        } else {
            warn!("poll_write reached end of pipeline");
            None
        }
    }

    /// Propagates timeout event to the next handler.
    ///
    /// Call this from [`Handler::handle_timeout`] to forward the timeout event
    /// through the pipeline.
    ///
    /// # Parameters
    ///
    /// - `now`: The current timestamp
    pub fn fire_handle_timeout(&self, now: Instant) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.handle_timeout_internal(context, now);
            }
        } else {
            warn!("handle_timeout reached end of pipeline");
        }
    }

    /// Polls the next handler for its timeout deadline.
    ///
    /// Call this from [`Handler::poll_timeout`] to allow the next handler to
    /// update the earliest timeout deadline.
    ///
    /// # Parameters
    ///
    /// - `eto`: Mutable reference to the earliest timeout. The next handler may update this.
    pub fn fire_poll_timeout(&self, eto: &mut Instant) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.poll_timeout_internal(context, eto);
            }
        } else {
            warn!("poll_timeout reached end of pipeline");
        }
    }

    /// Propagates EOF event to the next handler.
    ///
    /// Call this from [`Handler::handle_eof`] to forward the end-of-file event
    /// through the pipeline.
    pub fn fire_handle_eof(&self) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.handle_eof_internal(context);
            }
        } else {
            warn!("handle_eof reached end of pipeline");
        }
    }

    /// Propagates error event to the next handler.
    ///
    /// Call this from [`Handler::handle_error`] to forward the error through
    /// the pipeline.
    ///
    /// # Parameters
    ///
    /// - `err`: The error to propagate
    pub fn fire_handle_error(&self, err: Box<dyn Error>) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.handle_error_internal(context, err);
            }
        } else {
            warn!("handle_error reached end of pipeline");
        }
    }

    /// Propagates close event to the next handler.
    ///
    /// Call this from [`Handler::handle_close`] to forward the close request
    /// through the pipeline.
    pub fn fire_handle_close(&self) {
        if let (Some(next_handler), Some(next_context)) = (self.next_handler, self.next_context) {
            unsafe {
                let handler = &mut *next_handler;
                let context = &*next_context;
                handler.handle_close_internal(context);
            }
        } else {
            warn!("handle_close reached end of pipeline");
        }
    }
}

impl<Rin: 'static, Rout: 'static, Win: 'static, Wout: 'static> ContextInternal
    for Context<Rin, Rout, Win, Wout>
{
    fn fire_transport_active_internal(&self) {
        self.fire_transport_active();
    }
    fn fire_transport_inactive_internal(&self) {
        self.fire_transport_inactive();
    }
    fn fire_handle_read_internal(&self, msg: Box<dyn Any>) {
        if let Ok(msg) = msg.downcast::<Rout>() {
            self.fire_handle_read(*msg);
        } else {
            panic!("msg can't downcast::<Rout> in {} handler", self.name());
        }
    }
    fn fire_poll_write_internal(&self) -> Option<Box<dyn Any>> {
        if let Some(msg) = self.fire_poll_write() {
            Some(Box::new(msg))
        } else {
            None
        }
    }

    fn fire_handle_timeout_internal(&self, now: Instant) {
        self.fire_handle_timeout(now);
    }
    fn fire_poll_timeout_internal(&self, eto: &mut Instant) {
        self.fire_poll_timeout(eto);
    }

    fn fire_handle_eof_internal(&self) {
        self.fire_handle_eof();
    }
    fn fire_handle_error_internal(&self, err: Box<dyn Error>) {
        self.fire_handle_error(err);
    }
    fn fire_handle_close_internal(&self) {
        self.fire_handle_close();
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_next_context(&mut self, next_context: *mut dyn ContextInternal) {
        self.next_context = Some(next_context);
    }
    fn set_next_handler(&mut self, next_handler: *mut dyn HandlerInternal) {
        self.next_handler = Some(next_handler);
    }
    fn clear_next(&mut self) {
        self.next_context = None;
        self.next_handler = None;
    }
}
