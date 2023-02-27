use mio::event::Evented;
use mio::{Poll, PollOpt, Ready, Registration, SetReadiness, Token};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::error::Error;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::channel::metal_io::{
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

    fn handle_timeout(&self, now: Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.handle_timeout_internal(&*context, now);
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

/// InboundPipeline
pub trait InboundPipeline<R> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self);

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self);

    /// Reads a message.
    fn read(&self, msg: R);

    /// Reads an Error exception in one of its inbound operations.
    fn read_exception(&self, err: Box<dyn Error + Send + Sync>);

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
    fn write_exception(&self, err: Box<dyn Error + Send + Sync>);

    /// Writes a close event.
    fn close(&self);
}

/// Outbound Events
pub enum OutboundEvent<W> {
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
    events: Mutex<VecDeque<OutboundEvent<W>>>,
    internal: Mutex<PipelineInternal<R, W>>,
    registration: Registration,
    set_readiness: SetReadiness,
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> Default for Pipeline<R, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> Pipeline<R, W> {
    /// Creates a new Pipeline
    pub fn new() -> Self {
        let (registration, set_readiness) = Registration::new2();
        Self {
            events: Mutex::new(VecDeque::new()),
            internal: Mutex::new(PipelineInternal::new()),
            registration,
            set_readiness,
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

    /// Polls an outbound event.
    pub fn poll_outbound_event(&self) -> Option<OutboundEvent<W>> {
        let mut events = self.events.lock().unwrap();
        events.pop_front()
    }

    /// Handles an outbound event.
    pub fn handle_outbound_event(&self, evt: OutboundEvent<W>) {
        match evt {
            OutboundEvent::Write(msg) => {
                let internal = self.internal.lock().unwrap();
                internal.write(msg);
            }
            OutboundEvent::WriteException(err) => {
                let internal = self.internal.lock().unwrap();
                internal.write_exception(err);
            }
            OutboundEvent::Close => {
                let internal = self.internal.lock().unwrap();
                internal.close();
            }
        }
    }
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> InboundPipeline<R> for Pipeline<R, W> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self) {
        let internal = self.internal.lock().unwrap();
        internal.transport_active();
    }

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self) {
        let internal = self.internal.lock().unwrap();
        internal.transport_inactive();
    }

    /// Reads a message.
    fn read(&self, msg: R) {
        let internal = self.internal.lock().unwrap();
        internal.read(msg);
    }

    /// Reads an Error exception in one of its inbound operations.
    fn read_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let internal = self.internal.lock().unwrap();
        internal.read_exception(err);
    }

    /// Reads an EOF event.
    fn read_eof(&self) {
        let internal = self.internal.lock().unwrap();
        internal.read_eof();
    }

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant) {
        let internal = self.internal.lock().unwrap();
        internal.handle_timeout(now);
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    fn poll_timeout(&self, eto: &mut Instant) {
        let internal = self.internal.lock().unwrap();
        internal.poll_timeout(eto);
    }
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> OutboundPipeline<W> for Pipeline<R, W> {
    /// Writes a message.
    fn write(&self, msg: W) {
        {
            let mut events = self.events.lock().unwrap();
            events.push_back(OutboundEvent::Write(msg));
        }
        let _ = self.set_readiness.set_readiness(Ready::readable());
    }

    /// Writes an Error exception from one of its outbound operations.
    fn write_exception(&self, err: Box<dyn Error + Send + Sync>) {
        {
            let mut events = self.events.lock().unwrap();
            events.push_back(OutboundEvent::WriteException(err));
        }
        let _ = self.set_readiness.set_readiness(Ready::readable());
    }

    /// Writes a close event.
    fn close(&self) {
        {
            let mut events = self.events.lock().unwrap();
            events.push_back(OutboundEvent::Close);
        }
        let _ = self.set_readiness.set_readiness(Ready::readable());
    }
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> Evented for Pipeline<R, W> {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        self.registration.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        self.registration.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        poll.deregister(&self.registration)
    }
}
