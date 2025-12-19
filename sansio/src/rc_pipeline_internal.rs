use std::cell::{RefCell, UnsafeCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::{error::Error, io::ErrorKind, marker::PhantomData, time::Instant};

use crate::{
    Context,
    handler::Handler,
    handler_internal::{ContextInternal, HandlerInternal},
};
use crate::{NotifyCallback, RESERVED_PIPELINE_HANDLE_NAME};

/// Internal Rc-based pipeline implementation with interior mutability.
///
/// Unlike `PipelineInternal`, this version uses interior mutability to allow
/// `&self` methods, enabling shared ownership via `Rc<RcPipeline>`.
///
/// # Use Case
///
/// This is designed for scenarios where a single pipeline must be shared across
/// multiple contexts, such as UDP servers where one pipeline handles messages
/// from multiple peers.
///
/// # Interior Mutability
///
/// - **Handlers**: Wrapped in `UnsafeCell` for mutable access during message processing
/// - **Transmits**: Wrapped in `RefCell` for safe interior mutability
/// - **Write Notify**: Wrapped in `RefCell` for safe interior mutability
///
/// # Safety
///
/// The `UnsafeCell` usage is safe because:
/// - Single-threaded executor (no concurrent access)
/// - Handlers are only accessed sequentially during message processing
/// - No aliasing violations in practice
pub(crate) struct RcPipelineInternal<R, W> {
    /// Handler names for debugging and removal operations
    names: Vec<String>,

    /// Handler implementations stored as boxed trait objects with interior mutability
    handlers: Vec<UnsafeCell<Box<dyn HandlerInternal>>>,

    /// Context objects that enable handlers to communicate with each other
    contexts: Vec<Box<dyn ContextInternal>>,

    /// Queue of outbound messages to be written to the transport
    /// Wrapped in Rc to share with LastHandler
    transmits: Rc<RefCell<VecDeque<W>>>,

    /// Callback to notify the I/O layer when data is ready to write
    write_notify: RefCell<Option<NotifyCallback>>,

    /// Phantom data for the type parameters
    phantom: PhantomData<(R, W)>,
}

impl<R: 'static, W: 'static> RcPipelineInternal<R, W> {
    pub(crate) fn new() -> Self {
        let transmits = Rc::new(RefCell::new(VecDeque::new()));
        let last_handler: LastHandler<W> = LastHandler::new(Rc::clone(&transmits));
        let (name, handler, context) = last_handler.generate();
        Self {
            names: vec![name],
            handlers: vec![UnsafeCell::new(handler)],
            contexts: vec![context],
            transmits,
            write_notify: RefCell::new(None),
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
        self.handlers.insert(len - 1, UnsafeCell::new(handler));
        self.contexts.insert(len - 1, context);
    }

    pub(crate) fn add_front(&mut self, handler: impl Handler + 'static) {
        let (name, handler, context) = handler.generate();
        if name == RESERVED_PIPELINE_HANDLE_NAME {
            panic!("handle name {} is reserved", name);
        }

        self.names.insert(0, name);
        self.handlers.insert(0, UnsafeCell::new(handler));
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
    /// See `PipelineInternal::finalize` for detailed documentation.
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
                unsafe {
                    let next_handler_ptr =
                        (&mut **self.handlers[j + 1].get()) as *mut dyn HandlerInternal;
                    let next_context_ptr =
                        self.contexts[j + 1].as_mut() as *mut dyn ContextInternal;

                    self.contexts[j].set_next_handler(next_handler_ptr);
                    self.contexts[j].set_next_context(next_context_ptr);
                }
            }
        }

        // Note: LastHandler already has its transmits Rc set during construction
        // No need to do anything here
    }

    pub(crate) fn write(&self, msg: W) {
        // Push to the transmits queue
        self.transmits.borrow_mut().push_back(msg);

        // Notify the I/O layer that there's data to write
        if let Some(notify) = self.write_notify.borrow().as_ref() {
            notify();
        }
    }

    pub(crate) fn set_write_notify(&self, notify: NotifyCallback) {
        *self.write_notify.borrow_mut() = Some(notify);
    }

    pub(crate) fn transport_active(&self) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.transport_active_internal(context);
        }
    }

    pub(crate) fn transport_inactive(&self) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.transport_inactive_internal(context);
        }
    }

    pub(crate) fn handle_read(&self, msg: R) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.handle_read_internal(context, Box::new(msg));
        }
    }

    pub(crate) fn poll_write(&self) -> Option<R> {
        unsafe {
            let handler = &mut *self.handlers[0].get();
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
    }

    pub(crate) fn handle_timeout(&self, now: Instant) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.handle_timeout_internal(context, now);
        }
    }

    pub(crate) fn poll_timeout(&self, eto: &mut Instant) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.poll_timeout_internal(context, eto);
        }
    }

    pub(crate) fn handle_eof(&self) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.handle_eof_internal(context);
        }
    }

    pub(crate) fn handle_error(&self, err: Box<dyn Error>) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.handle_error_internal(context, err);
        }
    }

    pub(crate) fn handle_close(&self) {
        unsafe {
            let handler = &mut *self.handlers[0].get();
            let context = &*self.contexts[0];
            handler.handle_close_internal(context);
        }
    }
}

/// LastHandler for RcPipelineInternal
///
/// Holds a shared reference to the transmits queue via Rc<RefCell<VecDeque<W>>>.
/// This allows safe shared access with runtime borrow checking.
pub(crate) struct LastHandler<W> {
    transmits: Rc<RefCell<VecDeque<W>>>,
}

impl<W> LastHandler<W> {
    pub(crate) fn new(transmits: Rc<RefCell<VecDeque<W>>>) -> Self {
        Self { transmits }
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
        ctx.fire_handle_read(msg);
    }

    fn poll_write(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout> {
        let mut transmits = self.transmits.borrow_mut();
        transmits.pop_front()
    }
}
