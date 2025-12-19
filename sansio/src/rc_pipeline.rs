//! RcPipeline - Pipeline with interior mutability for shared ownership
//!
//! This module provides `RcPipeline`, a variant of `Pipeline` that uses
//! interior mutability to allow shared ownership via `Rc<RcPipeline>`.

use crate::NotifyCallback;
use crate::handler::Handler;
use crate::rc_pipeline_internal::RcPipelineInternal;
use std::{error::Error, time::Instant};

/// Inbound pipeline trait for Rc-wrapped pipelines (methods take `&self`).
///
/// This trait is similar to [`super::InboundPipeline`] but designed for pipelines
/// with interior mutability that can be shared via `Rc<RcPipeline>`.
pub trait RcInboundPipeline<R> {
    /// Notifies the pipeline that the transport is active (connected).
    fn transport_active(&self);

    /// Notifies the pipeline that the transport is inactive (disconnected).
    fn transport_inactive(&self);

    /// Handles an incoming message.
    fn handle_read(&self, msg: R);

    /// Polls the pipeline for an outgoing message.
    fn poll_write(&self) -> Option<R>;

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant);

    /// Polls for the earliest timeout needed by handlers.
    fn poll_timeout(&self, eto: &mut Instant);

    /// Handles an end-of-file event.
    fn handle_eof(&self);

    /// Handles an error event.
    fn handle_error(&self, err: Box<dyn Error>);

    #[doc(hidden)]
    fn set_write_notify(&self, notify: NotifyCallback);
}

/// Outbound pipeline trait for Rc-wrapped pipelines (methods take `&self`).
///
/// This trait is similar to [`super::OutboundPipeline`] but designed for pipelines
/// with interior mutability that can be shared via `Rc<RcPipeline>`.
pub trait RcOutboundPipeline<W> {
    /// Writes a message to the pipeline.
    fn write(&self, msg: W);

    /// Initiates pipeline close.
    fn close(&self);
}

/// A pipeline with interior mutability, designed for Rc-based shared ownership.
///
/// Unlike [`super::Pipeline`], `RcPipeline` uses interior mutability (`RefCell`, `UnsafeCell`)
/// to allow methods to take `&self` instead of `&mut self`. This enables wrapping in
/// `Rc<RcPipeline>` for scenarios where a single pipeline must be shared across
/// multiple contexts.
///
/// # Use Cases
///
/// - **UDP servers**: One pipeline shared across messages from multiple peers
/// - **Broadcast/multicast**: Multiple handlers/contexts need to write to the same pipeline
/// - **Shared state**: When the pipeline itself needs to be part of shared state
///
/// # Comparison with Pipeline
///
/// | Feature | Pipeline | RcPipeline |
/// |---------|----------|----------------|
/// | Ownership | Exclusive (`&mut self`) | Shared (`Rc<RcPipeline>`) |
/// | Methods | `&mut self` | `&self` |
/// | Overhead | Zero | RefCell + UnsafeCell |
/// | Use case | TCP (per-connection) | UDP (shared) |
///
/// # Example
///
/// ```rust
/// use sansio::{RcPipeline, RcInboundPipeline, Handler, Context};
/// use std::rc::Rc;
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
/// let mut pipeline = RcPipeline::<String, String>::new();
/// pipeline.add_back(MyHandler);
/// pipeline.finalize();
///
/// // Wrap in Rc for sharing
/// let pipeline = Rc::new(pipeline);
///
/// // Can now share across multiple contexts
/// let pipeline_ref1 = Rc::clone(&pipeline);
/// let pipeline_ref2 = Rc::clone(&pipeline);
///
/// // Both can call methods (using &self)
/// pipeline_ref1.handle_read("Hello".to_string());
/// pipeline_ref2.handle_read("World".to_string());
/// ```
///
/// # Safety
///
/// Internally uses `UnsafeCell` for handlers. This is safe because:
/// - Single-threaded executor (no concurrent access)
/// - Handlers are only accessed sequentially during message processing
/// - No aliasing violations in practice
pub struct RcPipeline<R, W> {
    internal: RcPipelineInternal<R, W>,
}

impl<R: 'static, W: 'static> Default for RcPipeline<R, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: 'static, W: 'static> RcPipeline<R, W> {
    /// Creates a new empty Rc-based pipeline.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sansio::RcPipeline;
    ///
    /// let pipeline: RcPipeline<Vec<u8>, Vec<u8>> = RcPipeline::new();
    /// ```
    pub fn new() -> Self {
        Self {
            internal: RcPipelineInternal::new(),
        }
    }

    /// Adds a handler to the end of the pipeline.
    ///
    /// Returns `&mut self` for method chaining.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use sansio::{RcPipeline, Handler, Context};
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
    /// let mut pipeline: RcPipeline<String, String> = RcPipeline::new();
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

impl<R: 'static, W: 'static> RcInboundPipeline<R> for RcPipeline<R, W> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self) {
        self.internal.transport_active();
    }

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self) {
        self.internal.transport_inactive();
    }

    /// Handles an incoming message.
    fn handle_read(&self, msg: R) {
        self.internal.handle_read(msg);
    }

    /// Polls an outgoing message
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
    fn handle_error(&self, err: Box<dyn Error>) {
        self.internal.handle_error(err);
    }

    #[doc(hidden)]
    fn set_write_notify(&self, notify: NotifyCallback) {
        self.internal.set_write_notify(notify);
    }
}

impl<R: 'static, W: 'static> RcOutboundPipeline<W> for RcPipeline<R, W> {
    /// Writes a message to pipeline
    fn write(&self, msg: W) {
        self.internal.write(msg);
    }

    /// Writes a close event.
    fn close(&self) {
        self.internal.handle_close();
    }
}
