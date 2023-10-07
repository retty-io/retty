use std::{cell::RefCell, error::Error, rc::Rc, time::Instant};

use crate::channel::{handler::Handler, pipeline_internal::PipelineInternal};
use crate::executor::yield_local;

/// InboundPipeline
pub trait InboundPipeline<R> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self);

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self);

    /// Reads a message.
    fn read(&self, msg: R);

    /// Reads an Error exception in one of its inbound operations.
    fn read_exception(&self, err: Box<dyn Error>);

    /// Reads an EOF event.
    fn read_eof(&self);

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant);

    /// Polls an event.
    fn poll_timeout(&self, eto: &mut Instant);
}

/// OutboundPipeline
pub trait OutboundPipeline<W> {
    /// Writes a message.
    fn write(&self, msg: W);

    /// Writes an Error exception from one of its outbound operations.
    fn write_exception(&self, err: Box<dyn Error>);

    /// Writes a close event.
    fn close(&self);

    /// Writes a message and yield to run other tasks.
    /// Usually call it in client pipeline
    fn write_and_yield(&self, msg: W) {
        self.write(msg);
        yield_local();
    }

    /// Writes an Error exception from one of its outbound operations
    /// and yield to run other tasks.
    /// Usually call it in client pipeline
    fn write_exception_and_yield(&self, err: Box<dyn Error>) {
        self.write_exception(err);
        yield_local();
    }

    /// Writes a close event and yield local to run other tasks.
    /// Usually call it in client pipeline
    fn close_and_yield(&self) {
        self.close();
        yield_local();
    }
}

/// Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
/// over how an event is handled and how the Handlers in a pipeline interact with each other.
pub struct Pipeline<R, W> {
    internal: RefCell<PipelineInternal<R, W>>,
}

impl<R: 'static, W: 'static> Default for Pipeline<R, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: 'static, W: 'static> Pipeline<R, W> {
    /// Creates a new Pipeline
    pub fn new() -> Self {
        Self {
            internal: RefCell::new(PipelineInternal::new()),
        }
    }

    /// Appends a [Handler] at the last position of this pipeline.
    pub fn add_back(&self, handler: impl Handler) -> &Self {
        {
            let mut internal = self.internal.borrow_mut();
            internal.add_back(handler);
        }
        self
    }

    /// Inserts a [Handler] at the first position of this pipeline.
    pub fn add_front(&self, handler: impl Handler) -> &Self {
        {
            let mut internal = self.internal.borrow_mut();
            internal.add_front(handler);
        }
        self
    }

    /// Removes a [Handler] at the last position of this pipeline.
    pub fn remove_back(&self) -> Result<&Self, std::io::Error> {
        let result = {
            let mut internal = self.internal.borrow_mut();
            internal.remove_back()
        };
        match result {
            Ok(()) => Ok(self),
            Err(err) => Err(err),
        }
    }

    /// Removes a [Handler] at the first position of this pipeline.
    pub fn remove_front(&self) -> Result<&Self, std::io::Error> {
        let result = {
            let mut internal = self.internal.borrow_mut();
            internal.remove_front()
        };
        match result {
            Ok(()) => Ok(self),
            Err(err) => Err(err),
        }
    }

    /// Removes a [Handler] from this pipeline based on handler_name.
    pub fn remove(&self, handler_name: &str) -> Result<&Self, std::io::Error> {
        let result = {
            let mut internal = self.internal.borrow_mut();
            internal.remove(handler_name)
        };
        match result {
            Ok(()) => Ok(self),
            Err(err) => Err(err),
        }
    }

    #[allow(clippy::len_without_is_empty)]
    /// Returns the number of Handlers in this pipeline.
    pub fn len(&self) -> usize {
        let internal = self.internal.borrow();
        internal.len()
    }

    /// Updates the Arc version's pipeline.
    pub fn update(self: Rc<Self>) -> Rc<Self> {
        {
            let internal = self.internal.borrow();
            internal.finalize();
        }
        self
    }

    /// Finalizes the pipeline.
    pub fn finalize(self) -> Rc<Self> {
        let pipeline = Rc::new(self);
        pipeline.update()
    }
}

impl<R: 'static, W: 'static> InboundPipeline<R> for Pipeline<R, W> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self) {
        let internal = self.internal.borrow();
        internal.transport_active();
    }

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self) {
        let internal = self.internal.borrow();
        internal.transport_inactive();
    }

    /// Reads a message.
    fn read(&self, msg: R) {
        let internal = self.internal.borrow();
        internal.read(msg);
    }

    /// Reads an Error exception in one of its inbound operations.
    fn read_exception(&self, err: Box<dyn Error>) {
        let internal = self.internal.borrow();
        internal.read_exception(err);
    }

    /// Reads an EOF event.
    fn read_eof(&self) {
        let internal = self.internal.borrow();
        internal.read_eof();
    }

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant) {
        let internal = self.internal.borrow();
        internal.handle_timeout(now);
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    fn poll_timeout(&self, eto: &mut Instant) {
        let internal = self.internal.borrow();
        internal.poll_timeout(eto);
    }
}

impl<R: 'static, W: 'static> OutboundPipeline<W> for Pipeline<R, W> {
    /// Writes a message.
    fn write(&self, msg: W) {
        let internal = self.internal.borrow();
        internal.write(msg);
    }

    /// Writes an Error exception from one of its outbound operations.
    fn write_exception(&self, err: Box<dyn Error>) {
        let internal = self.internal.borrow();
        internal.write_exception(err);
    }

    /// Writes a close event.
    fn close(&self) {
        let internal = self.internal.borrow();
        internal.close();
    }
}
