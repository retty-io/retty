//! ArcPipeline - Thread-safe pipeline with Mutex-based synchronization
//!
//! This module provides `ArcPipeline`, a thread-safe variant that can be
//! shared across threads using `Arc<ArcPipeline>`.

use crate::NotifyCallback;
use crate::arc_pipeline_internal::ArcPipelineInternal;
use crate::handler::Handler;
use std::{error::Error, time::Instant};

/// Inbound pipeline trait for Arc-wrapped pipelines.
///
/// This trait is similar to [`super::InboundPipeline`] but designed for pipelines
/// that can be shared across threads via `Arc<ArcPipeline>`.
///
/// All message types must implement `Send` to be safely sent between threads.
pub trait ArcInboundPipeline<R: Send>: Send + Sync {
    /// Notifies the pipeline that the transport is active (connected).
    fn transport_active(&self);

    /// Notifies the pipeline that the transport is inactive (disconnected).
    fn transport_inactive(&self);

    /// Handles an incoming message.
    ///
    /// Note: This is **serialized** - only one thread can process messages at a time.
    fn handle_read(&self, msg: R);

    /// Polls the pipeline for an outgoing message.
    ///
    /// Note: This is **serialized** - only one thread can poll at a time.
    fn poll_write(&self) -> Option<R>;

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant);

    /// Polls for the earliest timeout needed by handlers.
    fn poll_timeout(&self, eto: &mut Instant);

    /// Handles an end-of-file event.
    fn handle_eof(&self);

    /// Handles an error event.
    fn handle_error(&self, err: Box<dyn Error + Send>);

    #[doc(hidden)]
    fn set_write_notify(&self, notify: NotifyCallback);
}

/// Outbound pipeline trait for Arc-wrapped pipelines.
///
/// This trait is similar to [`super::OutboundPipeline`] but designed for pipelines
/// that can be shared across threads via `Arc<ArcPipeline>`.
///
/// All message types must implement `Send` to be safely sent between threads.
pub trait ArcOutboundPipeline<W: Send>: Send + Sync {
    /// Writes a message to the pipeline.
    ///
    /// This can be called concurrently from multiple threads.
    fn write(&self, msg: W);

    /// Initiates pipeline close.
    fn close(&self);
}

/// A thread-safe pipeline that can be shared across threads.
///
/// Unlike [`super::Pipeline`] and [`super::RcPipeline`], `ArcPipeline`
/// uses `Mutex` for synchronization, allowing it to be shared via `Arc` across
/// multiple threads.
///
/// # Thread Safety
///
/// - **Processing is serialized**: Only one thread can call `handle_read()` / `poll_write()` at a time
/// - **Writing is concurrent**: Multiple threads can call `write()` simultaneously
/// - **All types must be `Send`**: Message types R and W must implement `Send`
///
/// # Use Cases
///
/// - **Multi-threaded servers**: Share one pipeline across multiple worker threads
/// - **Cross-thread communication**: Send pipeline to another thread for processing
/// - **Async runtimes without LocalSet**: When you can't use `Rc`
///
/// # Comparison
///
/// | Feature | Pipeline | RcPipeline | ArcPipeline |
/// |---------|----------|------------|-------------|
/// | Ownership | Exclusive | Shared (Rc) | Shared (Arc) |
/// | Methods | `&mut self` | `&self` | `&self` |
/// | Sync | RefCell | RefCell/UnsafeCell | Mutex |
/// | Thread-safe | No | No | Yes |
/// | Processing | Direct | Direct | Serialized (Mutex) |
/// | Use case | TCP | UDP (single-threaded) | Multi-threaded |
///
/// # Example
///
/// ```rust
/// use sansio::{ArcPipeline, ArcInboundPipeline, Handler, Context};
/// use std::sync::Arc;
/// use std::thread;
///
/// # struct MyHandler;
/// # impl Handler for MyHandler {
/// #     type Rin = String;
/// #     type Rout = String;
/// #     type Win = String;
/// #     type Wout = String;
/// #     fn name(&self) -> &str { "MyHandler" }
/// #     fn handle_read(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, msg: Self::Rin) {
/// #         ctx.fire_handle_read(msg);
/// #     }
/// #     fn poll_write(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> {
/// #         ctx.fire_poll_write()
/// #     }
/// # }
///
/// // Create and configure pipeline
/// let mut pipeline = ArcPipeline::<String, String>::new();
/// pipeline.add_back(MyHandler);
/// pipeline.finalize();
///
/// // Wrap in Arc for cross-thread sharing
/// let pipeline = Arc::new(pipeline);
///
/// // Send to another thread
/// let pipeline_clone = Arc::clone(&pipeline);
/// thread::spawn(move || {
///     pipeline_clone.handle_read("Hello from thread".to_string());
/// });
///
/// // Use from main thread
/// pipeline.handle_read("Hello from main".to_string());
/// ```
///
/// # Performance
///
/// Processing is **serialized** - only one thread processes messages at a time.
/// This is necessary to maintain the handler chain invariants and prevent data races.
///
/// If you need truly parallel processing, consider:
/// - Creating separate pipeline instances per thread
/// - Using a work-stealing queue before the pipeline
pub struct ArcPipeline<R, W>
where
    R: Send + 'static,
    W: Send + 'static,
{
    internal: ArcPipelineInternal<R, W>,
}

// Explicit Send + Sync implementations
unsafe impl<R: Send, W: Send> Send for ArcPipeline<R, W> {}
unsafe impl<R: Send, W: Send> Sync for ArcPipeline<R, W> {}

impl<R: Send + 'static, W: Send + 'static> Default for ArcPipeline<R, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Send + 'static, W: Send + 'static> ArcPipeline<R, W> {
    /// Creates a new empty thread-safe pipeline.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sansio::ArcPipeline;
    ///
    /// let pipeline: ArcPipeline<Vec<u8>, Vec<u8>> = ArcPipeline::new();
    /// ```
    pub fn new() -> Self {
        Self {
            internal: ArcPipelineInternal::new(),
        }
    }

    /// Adds a handler to the end of the pipeline.
    ///
    /// Returns `&mut self` for method chaining.
    ///
    /// **Note**: This is not thread-safe. Build your pipeline before wrapping in `Arc`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{ArcPipeline, Handler, Context};
    /// # struct H1;
    /// # impl Handler for H1 {
    /// #     type Rin = String;
    /// #     type Rout = String;
    /// #     type Win = String;
    /// #     type Wout = String;
    /// #     fn name(&self) -> &str { "H1" }
    /// #     fn handle_read(&mut self, _: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>, _: Self::Rin) {}
    /// #     fn poll_write(&mut self, _: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) -> Option<Self::Wout> { None }
    /// # }
    /// let mut pipeline: ArcPipeline<String, String> = ArcPipeline::new();
    /// pipeline.add_back(H1);
    /// ```
    pub fn add_back(&mut self, handler: impl Handler + 'static) -> &mut Self {
        self.internal.add_back(handler);
        self
    }

    /// Adds a handler to the beginning of the pipeline.
    ///
    /// Returns `&mut self` for method chaining.
    pub fn add_front(&mut self, handler: impl Handler + 'static) -> &mut Self {
        self.internal.add_front(handler);
        self
    }

    /// Removes the last handler from the pipeline.
    pub fn remove_back(&mut self) -> Result<&mut Self, std::io::Error> {
        let result = self.internal.remove_back();
        match result {
            Ok(()) => Ok(self),
            Err(err) => Err(err),
        }
    }

    /// Removes the first handler from the pipeline.
    pub fn remove_front(&mut self) -> Result<&mut Self, std::io::Error> {
        let result = self.internal.remove_front();
        match result {
            Ok(()) => Ok(self),
            Err(err) => Err(err),
        }
    }

    /// Removes all handlers with the given name from the pipeline.
    pub fn remove(&mut self, handler_name: &str) -> Result<&mut Self, std::io::Error> {
        let result = self.internal.remove(handler_name);
        match result {
            Ok(()) => Ok(self),
            Err(err) => Err(err),
        }
    }

    #[allow(clippy::len_without_is_empty)]
    /// Returns the number of handlers in the pipeline.
    pub fn len(&self) -> usize {
        self.internal.len()
    }

    /// Finalize the pipeline, making it ready for use.
    ///
    /// **You must call this before using the pipeline.**
    pub fn finalize(&mut self) {
        self.internal.finalize();
    }
}

impl<R: Send + 'static, W: Send + 'static> ArcInboundPipeline<R> for ArcPipeline<R, W> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self) {
        self.internal.transport_active();
    }

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self) {
        self.internal.transport_inactive();
    }

    /// Handles an incoming message.
    ///
    /// Note: Processing is serialized via Mutex - only one thread can process at a time.
    fn handle_read(&self, msg: R) {
        self.internal.handle_read(msg);
    }

    /// Polls an outgoing message.
    ///
    /// Note: Polling is serialized via Mutex - only one thread can poll at a time.
    fn poll_write(&self) -> Option<R> {
        self.internal.poll_write()
    }

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant) {
        self.internal.handle_timeout(now);
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    fn poll_timeout(&self, eto: &mut Instant) {
        self.internal.poll_timeout(eto);
    }

    /// Handles an EOF event.
    fn handle_eof(&self) {
        self.internal.handle_eof();
    }

    /// Handles an error.
    fn handle_error(&self, err: Box<dyn Error + Send>) {
        self.internal.handle_error(err);
    }

    #[doc(hidden)]
    fn set_write_notify(&self, notify: NotifyCallback) {
        self.internal.set_write_notify(notify);
    }
}

impl<R: Send + 'static, W: Send + 'static> ArcOutboundPipeline<W> for ArcPipeline<R, W> {
    /// Writes a message to pipeline.
    ///
    /// This can be called concurrently from multiple threads.
    fn write(&self, msg: W) {
        self.internal.write(msg);
    }

    /// Writes a close event.
    fn close(&self) {
        self.internal.handle_close();
    }
}
