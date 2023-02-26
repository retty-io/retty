use std::cell::RefCell;
use std::collections::VecDeque;
use std::error::Error;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::channel::sync_io::{
    handler::Handler,
    handler_internal::{
        InboundContextInternal, InboundHandlerInternal, OutboundContextInternal,
        OutboundHandlerInternal,
    },
};

struct PipelineInternal<R, W> {
    handler_names: Vec<String>,

    inbound_handlers: Vec<Rc<RefCell<dyn InboundHandlerInternal>>>,
    inbound_contexts: Vec<Rc<RefCell<dyn InboundContextInternal>>>,

    outbound_handlers: Vec<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    outbound_contexts: Vec<Rc<RefCell<dyn OutboundContextInternal>>>,

    phantom: PhantomData<(R, W)>,
}

// Since PipelineInternal is already protected by Mutex in Pipeline(internal: Mutex<PipelineInternal<R, W>>),
// it is safe to use Rc<RefCell<<>> internally. So, here use unsafe to mark PipelineInternal as Send
unsafe impl<R, W> Send for PipelineInternal<R, W> {}

impl<R: 'static, W: 'static> PipelineInternal<R, W> {
    fn new() -> Self {
        Self {
            handler_names: Vec::new(),

            inbound_handlers: Vec::new(),
            inbound_contexts: Vec::new(),

            outbound_handlers: Vec::new(),
            outbound_contexts: Vec::new(),

            phantom: PhantomData,
        }
    }

    fn add_back(&mut self, handler: impl Handler) {
        let (handler_name, inbound_handler, inbound_context, outbound_handler, outbound_context) =
            handler.generate();
        self.handler_names.push(handler_name);

        self.inbound_handlers.push(inbound_handler);
        self.inbound_contexts.push(inbound_context);

        self.outbound_handlers.push(outbound_handler);
        self.outbound_contexts.push(outbound_context);
    }

    fn add_front(&mut self, handler: impl Handler) {
        let (handler_name, inbound_handler, inbound_context, outbound_handler, outbound_context) =
            handler.generate();
        self.handler_names.insert(0, handler_name);

        self.inbound_handlers.insert(0, inbound_handler);
        self.inbound_contexts.insert(0, inbound_context);

        self.outbound_handlers.insert(0, outbound_handler);
        self.outbound_contexts.insert(0, outbound_context);
    }

    fn remove_back(&mut self) -> Result<(), std::io::Error> {
        let len = self.handler_names.len();
        if len == 0 {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "No handlers in pipeline",
            ))
        } else {
            self.handler_names.remove(len - 1);

            self.inbound_handlers.remove(len - 1);
            self.inbound_contexts.remove(len - 1);

            self.outbound_handlers.remove(len - 1);
            self.outbound_contexts.remove(len - 1);

            Ok(())
        }
    }

    fn remove_front(&mut self) -> Result<(), std::io::Error> {
        if self.handler_names.is_empty() {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "No handlers in pipeline",
            ))
        } else {
            self.handler_names.remove(0);

            self.inbound_handlers.remove(0);
            self.inbound_contexts.remove(0);

            self.outbound_handlers.remove(0);
            self.outbound_contexts.remove(0);

            Ok(())
        }
    }

    fn remove(&mut self, handler_name: &str) -> Result<(), std::io::Error> {
        let mut to_be_removed = vec![];
        for (index, name) in self.handler_names.iter().enumerate() {
            if name == handler_name {
                to_be_removed.push(index);
            }
        }

        if !to_be_removed.is_empty() {
            for index in to_be_removed.into_iter().rev() {
                self.handler_names.remove(index);

                self.inbound_handlers.remove(index);
                self.inbound_contexts.remove(index);

                self.outbound_handlers.remove(index);
                self.outbound_contexts.remove(index);
            }

            Ok(())
        } else {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                format!("No such handler \"{}\" in pipeline", handler_name),
            ))
        }
    }

    fn len(&self) -> usize {
        self.handler_names.len()
    }

    fn finalize(&self) {
        let mut enumerate = self.inbound_contexts.iter().enumerate();
        let ctx_pipe_len = self.inbound_contexts.len();
        for _ in 0..ctx_pipe_len {
            let (j, ctx) = enumerate.next().unwrap();
            let mut curr = ctx.borrow_mut();

            {
                let (next_context, next_handler) = (
                    self.inbound_contexts.get(j + 1),
                    self.inbound_handlers.get(j + 1),
                );
                match (next_context, next_handler) {
                    (Some(next_ctx), Some(next_hdlr)) => {
                        curr.set_next_in_context(Some(next_ctx.clone()));
                        curr.set_next_in_handler(Some(next_hdlr.clone()));
                    }
                    _ => {
                        curr.set_next_in_context(None);
                        curr.set_next_in_handler(None);
                    }
                }
            }

            {
                let (prev_context, prev_handler) = if j > 0 {
                    (
                        self.outbound_contexts.get(j - 1),
                        self.outbound_handlers.get(j - 1),
                    )
                } else {
                    (None, None)
                };
                match (prev_context, prev_handler) {
                    (Some(prev_ctx), Some(prev_hdlr)) => {
                        curr.set_next_out_context(Some(prev_ctx.clone()));
                        curr.set_next_out_handler(Some(prev_hdlr.clone()));
                    }
                    _ => {
                        curr.set_next_out_context(None);
                        curr.set_next_out_handler(None);
                    }
                }
            }
        }

        let mut enumerate = self.outbound_contexts.iter().enumerate();
        let ctx_pipe_len = self.outbound_contexts.len();
        for _ in 0..ctx_pipe_len {
            let (j, ctx) = enumerate.next().unwrap();
            let mut curr = ctx.borrow_mut();

            {
                let (prev_context, prev_handler) = if j > 0 {
                    (
                        self.outbound_contexts.get(j - 1),
                        self.outbound_handlers.get(j - 1),
                    )
                } else {
                    (None, None)
                };
                match (prev_context, prev_handler) {
                    (Some(prev_ctx), Some(prev_hdlr)) => {
                        curr.set_next_out_context(Some(prev_ctx.clone()));
                        curr.set_next_out_handler(Some(prev_hdlr.clone()));
                    }
                    _ => {
                        curr.set_next_out_context(None);
                        curr.set_next_out_handler(None);
                    }
                }
            }
        }
    }

    fn transport_active(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.transport_active_internal(&*context);
    }

    fn transport_inactive(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.transport_inactive_internal(&*context);
    }

    fn read(&self, msg: R) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_internal(&*context, Box::new(msg));
    }

    fn read_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_exception_internal(&*context, err);
    }

    fn read_eof(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_eof_internal(&*context);
    }

    fn read_timeout(&self, now: Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_timeout_internal(&*context, now);
    }

    fn poll_timeout(&self, eto: &mut Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.poll_timeout_internal(&*context, eto);
    }

    fn write(&self, msg: W) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.write_internal(&*context, Box::new(msg));
    }

    fn write_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.write_exception_internal(&*context, err);
    }

    fn close(&self) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.close_internal(&*context);
    }
}

/// Pipeline Events
pub enum Event<R, W> {
    /// Read Event
    Read(R),
    /// ReadException Event
    ReadException(Box<dyn Error + Send + Sync>),
    /// ReadEof Event
    ReadEof,
    /// Write Event
    Write(W),
    /// WriteException Event
    WriteException(Box<dyn Error + Send + Sync>),
    /// Close Event
    Close,
}

/// Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
/// over how an event is handled and how the Handlers in a pipeline interact with each other.
pub struct Pipeline<R, W> {
    events: Mutex<VecDeque<Event<R, W>>>,
    internal: Mutex<PipelineInternal<R, W>>,
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> Default for Pipeline<R, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> Pipeline<R, W> {
    /// Creates a new Pipeline
    pub fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            internal: Mutex::new(PipelineInternal::new()),
        }
    }

    /// Appends a [Handler] at the last position of this pipeline.
    pub fn add_back(&self, handler: impl Handler) -> &Self {
        {
            let mut internal = self.internal.lock().unwrap();
            internal.add_back(handler);
        }
        self
    }

    /// Inserts a [Handler] at the first position of this pipeline.
    pub fn add_front(&self, handler: impl Handler) -> &Self {
        {
            let mut internal = self.internal.lock().unwrap();
            internal.add_front(handler);
        }
        self
    }

    /// Removes a [Handler] at the last position of this pipeline.
    pub fn remove_back(&self) -> Result<&Self, std::io::Error> {
        let result = {
            let mut internal = self.internal.lock().unwrap();
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
            let mut internal = self.internal.lock().unwrap();
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
            let mut internal = self.internal.lock().unwrap();
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
        let internal = self.internal.lock().unwrap();
        internal.len()
    }

    /// Updates the Arc version's pipeline.
    pub fn update(self: Arc<Self>) -> Arc<Self> {
        {
            let internal = self.internal.lock().unwrap();
            internal.finalize();
        }
        self
    }

    /// Finalizes the pipeline.
    pub fn finalize(self) -> Arc<Self> {
        let pipeline = Arc::new(self);
        pipeline.update()
    }

    /// Transport is active now, which means it is connected.
    pub fn transport_active(&self) {
        let internal = self.internal.lock().unwrap();
        internal.transport_active();
    }

    /// Transport is inactive now, which means it is disconnected.
    pub fn transport_inactive(&self) {
        let internal = self.internal.lock().unwrap();
        internal.transport_inactive();
    }

    /// Reads a message.
    pub fn read(&self, msg: R) {
        let mut events = self.events.lock().unwrap();
        events.push_back(Event::Read(msg));
    }

    /// Reads an Error exception in one of its inbound operations.
    pub fn read_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let mut events = self.events.lock().unwrap();
        events.push_back(Event::ReadException(err));
    }

    /// Reads an EOF event.
    pub fn read_eof(&self) {
        let mut events = self.events.lock().unwrap();
        events.push_back(Event::ReadEof);
    }

    /// Writes a message.
    pub fn write(&self, msg: W) {
        let mut events = self.events.lock().unwrap();
        events.push_back(Event::Write(msg));
    }

    /// Writes an Error exception from one of its outbound operations.
    pub fn write_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let mut events = self.events.lock().unwrap();
        events.push_back(Event::WriteException(err));
    }

    /// Writes a close event.
    pub fn close(&self) {
        let mut events = self.events.lock().unwrap();
        events.push_back(Event::Close);
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    pub fn poll_timeout(&self, eto: &mut Instant) {
        let internal = self.internal.lock().unwrap();
        internal.poll_timeout(eto);
    }

    /// Polls an event.
    pub fn poll_event(&self) -> Option<Event<R, W>> {
        let mut events = self.events.lock().unwrap();
        events.pop_front()
    }

    /// Handles a timeout event.
    pub fn handle_timeout(&self, now: Instant) {
        let internal = self.internal.lock().unwrap();
        internal.read_timeout(now);
    }

    /// Handles an event.
    pub fn handle_event(&self, evt: Event<R, W>) {
        match evt {
            Event::Read(msg) => {
                let internal = self.internal.lock().unwrap();
                internal.read(msg);
            }
            Event::ReadException(err) => {
                let internal = self.internal.lock().unwrap();
                internal.read_exception(err);
            }
            Event::ReadEof => {
                let internal = self.internal.lock().unwrap();
                internal.read_eof();
            }
            Event::Write(msg) => {
                let internal = self.internal.lock().unwrap();
                internal.write(msg);
            }
            Event::WriteException(err) => {
                let internal = self.internal.lock().unwrap();
                internal.write_exception(err);
            }
            Event::Close => {
                let internal = self.internal.lock().unwrap();
                internal.close();
            }
        }
    }
}
