use std::collections::VecDeque;
use std::{error::Error, io::ErrorKind, marker::PhantomData, time::Instant};

use crate::{
    Context,
    handler::Handler,
    handler_internal::{ContextInternal, HandlerInternal},
};
use crate::{NotifyCallback, RESERVED_PIPELINE_HANDLE_NAME};

/// Internal pipeline implementation that manages the handler chain.
///
/// # Architecture
///
/// The pipeline uses direct ownership (no `Rc`/`RefCell`) with raw pointers for handler chaining:
///
/// - **Handlers and Contexts**: Stored in `Vec<Box<dyn Trait>>` for stable addresses
/// - **Handler Chain**: Each context holds raw pointers (`*mut`) to the next handler/context
/// - **Transmits Queue**: Owned by the pipeline, accessed by LastHandler via raw pointer
///
/// # Memory Layout
///
/// ```text
/// PipelineInternal<R, W>
/// ├─ handlers: Vec<Box<dyn HandlerInternal>>
/// │  ├─ Box<dyn Handler> (user handler 1)
/// │  ├─ Box<dyn Handler> (user handler 2)
/// │  └─ Box<dyn Handler<W,W,W,W>> (LastHandler)
/// │
/// ├─ contexts: Vec<Box<dyn ContextInternal>>
/// │  ├─ Box<Context<...>> (for handler 1)
/// │  ├─ Box<Context<...>> (for handler 2)
/// │  └─ Box<Context<W,W,W,W>> (for LastHandler)
/// │
/// └─ transmits: VecDeque<W>
///    └─ Referenced by LastHandler via *mut VecDeque<W>
/// ```
///
/// # Safety
///
/// Raw pointers are safe here because:
/// - The pipeline owns all handlers/contexts for their entire lifetime
/// - Pointers are only created during `finalize()` and remain valid until re-finalize
/// - `Box<dyn Trait>` ensures stable addresses even if the Vec reallocates
/// - Single-threaded use only (not thread-safe)
///
/// # Handler Linking
///
/// During `finalize()`:
/// 1. Clear all existing handler links (set pointers to null)
/// 2. Create new raw pointer links between adjacent handlers
/// 3. Set LastHandler's transmits pointer to point to the pipeline's transmits queue
///
/// # Writing Messages
///
/// When `write(msg)` is called:
/// 1. Pipeline directly pushes the message to its `transmits` queue
/// 2. Optionally notifies the I/O layer that data is ready
///
/// When `poll_write()` is called:
/// 1. Pipeline calls the first handler's `poll_write_internal()`
/// 2. This cascades through the handler chain
/// 3. LastHandler (at the end) pops from the transmits queue via its raw pointer
/// 4. Messages flow back through the chain in reverse order
pub(crate) struct PipelineInternal<R, W> {
    /// Handler names for debugging and removal operations
    names: Vec<String>,

    /// Handler implementations stored as boxed trait objects
    handlers: Vec<Box<dyn HandlerInternal>>,

    /// Context objects that enable handlers to communicate with each other
    contexts: Vec<Box<dyn ContextInternal>>,

    /// Queue of outbound messages to be written to the transport
    transmits: VecDeque<W>,

    /// Callback to notify the I/O layer when data is ready to write
    write_notify: Option<NotifyCallback>,

    /// Phantom data for the type parameters
    phantom: PhantomData<(R, W)>,
}

impl<R: 'static, W: 'static> PipelineInternal<R, W> {
    pub(crate) fn new() -> Self {
        let last_handler: LastHandler<W> = LastHandler::new();
        let (name, handler, context) = last_handler.generate();
        Self {
            names: vec![name],
            handlers: vec![handler],
            contexts: vec![context],
            transmits: VecDeque::new(),
            write_notify: None,
            phantom: PhantomData,
        }
    }

    pub(crate) fn add_back(&mut self, handler: impl Handler + 'static) {
        let (name, handler, context) = handler.generate();
        if name == RESERVED_PIPELINE_HANDLE_NAME {
            panic!("handle name {} is reserved", name);
        }

        let len = self.names.len();

        self.names.insert(len - 1, name);
        self.handlers.insert(len - 1, handler);
        self.contexts.insert(len - 1, context);
    }

    pub(crate) fn add_front(&mut self, handler: impl Handler + 'static) {
        let (name, handler, context) = handler.generate();
        if name == RESERVED_PIPELINE_HANDLE_NAME {
            panic!("handle name {} is reserved", name);
        }

        self.names.insert(0, name);
        self.handlers.insert(0, handler);
        self.contexts.insert(0, context);
    }

    pub(crate) fn remove_back(&mut self) -> Result<(), std::io::Error> {
        let len = self.names.len();
        if len == 1 {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "No handlers in pipeline",
            ))
        } else {
            self.names.remove(len - 2);
            self.handlers.remove(len - 2);
            self.contexts.remove(len - 2);

            Ok(())
        }
    }

    pub(crate) fn remove_front(&mut self) -> Result<(), std::io::Error> {
        let len = self.names.len();
        if len == 1 {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "No handlers in pipeline",
            ))
        } else {
            self.names.remove(0);
            self.handlers.remove(0);
            self.contexts.remove(0);

            Ok(())
        }
    }

    pub(crate) fn remove(&mut self, handler_name: &str) -> Result<(), std::io::Error> {
        if handler_name == RESERVED_PIPELINE_HANDLE_NAME {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                format!("handle name {} is reserved", handler_name),
            ));
        }

        let mut to_be_removed = vec![];
        for (index, name) in self.names.iter().enumerate() {
            if name == handler_name {
                to_be_removed.push(index);
            }
        }

        if !to_be_removed.is_empty() {
            for index in to_be_removed.into_iter().rev() {
                self.names.remove(index);
                self.handlers.remove(index);
                self.contexts.remove(index);
            }

            Ok(())
        } else {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                format!("No such handler \"{}\" in pipeline", handler_name),
            ))
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.names.len() - 1
    }

    /// Finalizes the pipeline by linking all handlers together.
    ///
    /// This method must be called after adding/removing handlers and before using the pipeline.
    /// It establishes the handler chain by:
    ///
    /// 1. Clearing any existing links between handlers
    /// 2. Setting raw pointers in each context to point to the next handler/context
    /// 3. Configuring LastHandler with a pointer to the transmits queue
    ///
    /// # Handler Chain Architecture
    ///
    /// Each `Context<Rin, Rout, Win, Wout>` holds raw pointers to:
    /// - `next_handler: *mut dyn HandlerInternal` - The next handler in the chain
    /// - `next_context: *mut dyn ContextInternal` - The next handler's context
    ///
    /// When a handler calls `ctx.fire_handle_read(msg)`, the context:
    /// 1. Dereferences the raw pointers (unsafe)
    /// 2. Calls `next_handler.handle_read_internal(next_context, msg)`
    /// 3. The next handler processes the message and potentially forwards it further
    ///
    /// # LastHandler Configuration
    ///
    /// The LastHandler (always the last handler in the chain) needs access to the
    /// transmits queue to return outbound messages. We configure this in three steps:
    ///
    /// 1. Get a pointer to the pipeline's transmits queue: `&mut self.transmits as *mut VecDeque<W>`
    /// 2. Downcast the last handler from `Box<dyn HandlerInternal>` to `Box<dyn Handler<W,W,W,W>>` using `as_any_mut()`
    /// 3. Extract the data pointer from the inner `dyn Handler` fat pointer and cast to `*mut LastHandler<W>`
    /// 4. Set `LastHandler::transmits` field to point to the pipeline's transmits queue
    ///
    /// This approach avoids the "invalid trait object cast" error by:
    /// - Downcasting the Box wrapper (a concrete type), not the trait object itself
    /// - Using pointer manipulation to extract the data pointer from the fat pointer
    /// - Directly accessing the concrete `LastHandler<W>` type through the raw pointer
    ///
    /// # Safety
    ///
    /// The raw pointers created here are safe because:
    /// - All handlers/contexts are owned by this pipeline and live as long as it does
    /// - Box provides stable addresses, so pointers remain valid even if Vec reallocates
    /// - Methods that modify the handler list (`add_back`, `remove`, etc.) call finalize again
    /// - The last handler is always `LastHandler<W>` by construction
    /// - The pointer cast assumes the fat pointer data component points to `LastHandler<W>`, which
    ///   is guaranteed by the pipeline's construction (only `LastHandler` is added at the end)
    /// - The pipeline is not thread-safe, so no concurrent access is possible
    pub(crate) fn finalize(&mut self) {
        // Clear all existing links first
        for ctx in self.contexts.iter_mut() {
            ctx.clear_next();
        }

        // Create new links using raw pointers
        for j in 0..self.contexts.len() {
            if j + 1 < self.contexts.len() {
                // Get raw pointers to the next handler and context
                // Safe because Box provides stable addresses
                let next_handler_ptr = self.handlers[j + 1].as_mut() as *mut dyn HandlerInternal;
                let next_context_ptr = self.contexts[j + 1].as_mut() as *mut dyn ContextInternal;

                self.contexts[j].set_next_handler(next_handler_ptr);
                self.contexts[j].set_next_context(next_context_ptr);
            }
        }

        // Set transmits pointer in LastHandler
        // LastHandler is always the last handler in the handlers vector
        if let Some(last_handler) = self.handlers.last_mut() {
            let transmits_ptr = &mut self.transmits as *mut VecDeque<W>;

            // Step 1: Downcast Box<dyn HandlerInternal> to Box<dyn Handler<W,W,W,W>>
            // This uses as_any_mut() which returns the Box itself as &mut dyn Any.
            // We can then downcast this concrete Box type to the specific Handler type.
            if let Some(handler_box) = last_handler
                .as_any_mut()
                .downcast_mut::<Box<dyn Handler<Rin = W, Rout = W, Win = W, Wout = W>>>()
            {
                // Step 2: Get a reference to the inner trait object
                // handler_box is &mut Box<dyn Handler>, so **handler_box gets us &mut dyn Handler
                let inner_handler: &mut dyn Handler<Rin = W, Rout = W, Win = W, Wout = W> =
                    &mut **handler_box;

                // Step 3: Extract the data pointer from the fat pointer
                // Fat pointers consist of (data_ptr, vtable_ptr)
                // We convert &mut dyn Handler to a raw pointer, then extract just the data part
                let handler_ptr =
                    inner_handler as *mut dyn Handler<Rin = W, Rout = W, Win = W, Wout = W>;

                // Step 4: Cast the data pointer to the concrete type
                // We cast through *mut () to strip the trait object metadata,
                // then cast to *mut LastHandler<W> since we know the last handler is always LastHandler
                let data_ptr = handler_ptr as *mut () as *mut LastHandler<W>;

                // Step 5: Set the transmits pointer in the concrete LastHandler struct
                // This is safe because:
                // - The pipeline owns both the transmits queue and the LastHandler
                // - The pointer remains valid until the pipeline is dropped or re-finalized
                // - LastHandler is the only handler type that appears at the end of the chain
                unsafe {
                    (*data_ptr).transmits = transmits_ptr;
                }
            }
        }
    }

    pub(crate) fn write(&mut self, msg: W) {
        // Simply push to the pipeline's transmits queue
        self.transmits.push_back(msg);

        // Notify the I/O layer that there's data to write
        if let Some(notify) = &self.write_notify {
            notify();
        }
    }

    pub(crate) fn set_write_notify(&mut self, notify: NotifyCallback) {
        self.write_notify = Some(notify);
    }

    pub(crate) fn transport_active(&mut self) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.transport_active_internal(context);
    }

    pub(crate) fn transport_inactive(&mut self) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.transport_inactive_internal(context);
    }

    pub(crate) fn handle_read(&mut self, msg: R) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.handle_read_internal(context, Box::new(msg));
    }

    pub(crate) fn poll_write(&mut self) -> Option<R> {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        if let Some(msg) = handler.poll_write_internal(context) {
            if let Ok(msg) = msg.downcast::<R>() {
                Some(*msg)
            } else {
                panic!("msg can't downcast::<R> in {} handler", context.name());
            }
        } else {
            None
        }
    }

    pub(crate) fn handle_timeout(&mut self, now: Instant) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.handle_timeout_internal(context, now);
    }

    pub(crate) fn poll_timeout(&mut self, eto: &mut Instant) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.poll_timeout_internal(context, eto);
    }

    pub(crate) fn handle_eof(&mut self) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.handle_eof_internal(context);
    }

    pub(crate) fn handle_error(&mut self, err: Box<dyn Error>) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.handle_error_internal(context, err);
    }

    pub(crate) fn handle_close(&mut self) {
        let handler = &mut *self.handlers[0];
        let context = &*self.contexts[0];
        handler.handle_close_internal(context);
    }
}

/// The last handler in every pipeline, responsible for interfacing with the transmits queue.
///
/// `LastHandler` is automatically added to every pipeline and cannot be removed. It serves as
/// the terminus of the handler chain, providing the final link between the pipeline's internal
/// logic and the outbound message queue.
///
/// # Architecture
///
/// Unlike other handlers, LastHandler holds a raw pointer to the pipeline's `transmits` queue:
///
/// ```text
/// ┌──────────────────────────────────────────────────────────────┐
/// │ PipelineInternal<R, W>                                       │
/// │                                                               │
/// │  handlers: Vec<Box<dyn HandlerInternal>>                     │
/// │    ├─ UserHandler1                                           │
/// │    ├─ UserHandler2                                           │
/// │    └�� LastHandler<W> ──────────┐                            │
/// │                                  │                            │
/// │  transmits: VecDeque<W> <───────┘ (raw pointer)             │
/// └──────────────────────────────────────────────────────────────┘
/// ```
///
/// # Pointer Management
///
/// The `transmits` pointer is set during pipeline finalization using a combination of
/// `as_any_mut()` downcasting and raw pointer manipulation:
///
/// 1. Pipeline calls `finalize()` after handlers are added/removed
/// 2. `finalize()` uses `as_any_mut()` to downcast `Box<dyn HandlerInternal>` to `Box<dyn Handler<W,W,W,W>>`
/// 3. Extracts the data pointer from the inner `dyn Handler` fat pointer
/// 4. Casts to `*mut LastHandler<W>` and sets the transmits field directly
///
/// This approach avoids the "invalid trait object cast" error by:
/// - Downcasting the Box wrapper (concrete type) instead of the trait object
/// - Using pointer manipulation to access the concrete type's fields
/// - Relying on the guarantee that the last handler is always `LastHandler<W>`
///
/// # Safety
///
/// The raw pointer is safe because:
/// - The pointer is only set during `finalize()` by the owning pipeline
/// - The pipeline owns both LastHandler and transmits for their entire lifetime
/// - The pointer remains valid until the pipeline is dropped or re-finalized
/// - LastHandler is always the last handler in the chain by construction
/// - Single-threaded use only (not thread-safe)
///
/// # Handler Behavior
///
/// - **`handle_read`**: Passes messages through to next handler (end of inbound chain)
/// - **`poll_write`**: Pops messages from the pipeline's transmits queue via raw pointer
/// - **Other events**: Propagate to next handler (no-op at end of pipeline)
pub(crate) struct LastHandler<W> {
    /// Raw pointer to the pipeline's transmits queue.
    ///
    /// This pointer is null until `finalize()` is called, at which point
    /// it points to the `transmits` field in `PipelineInternal`.
    transmits: *mut VecDeque<W>,
}

impl<W> LastHandler<W> {
    /// Creates a new LastHandler with a null transmits pointer.
    ///
    /// The pointer will be set when `finalize()` is called on the pipeline.
    pub(crate) fn new() -> Self {
        Self {
            transmits: std::ptr::null_mut(),
        }
    }
}

impl<W: 'static> Handler for LastHandler<W> {
    type Rin = W;
    type Rout = Self::Rin;
    type Win = Self::Rin;
    type Wout = Self::Rin;

    fn name(&self) -> &str {
        RESERVED_PIPELINE_HANDLE_NAME
    }

    fn handle_read(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    ) {
        // LastHandler is the terminus of the inbound chain.
        // It propagates messages to the next context (which will be null),
        // causing the message to be dropped. This is intentional - LastHandler
        // only processes outbound messages via poll_write().
        ctx.fire_handle_read(msg);
    }

    /// Polls the transmits queue for outbound messages.
    ///
    /// Returns the next message from the pipeline's transmits queue (via raw pointer), or None if empty.
    ///
    /// # Safety
    ///
    /// Uses unsafe to dereference the transmits pointer. This is safe because:
    /// - The pointer is set by the owning pipeline during `finalize()`
    /// - The pipeline owns both this handler and the transmits queue
    /// - The pointer remains valid for the handler's lifetime
    fn poll_write(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout> {
        if !self.transmits.is_null() {
            unsafe {
                let transmits = &mut *self.transmits;
                transmits.pop_front()
            }
        } else {
            None
        }
    }
}
