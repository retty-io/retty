use std::{cell::RefCell, error::Error, io::ErrorKind, marker::PhantomData, rc::Rc, time::Instant};

use crate::channel::{
    handler::Handler,
    handler_internal::{
        InboundContextInternal, InboundHandlerInternal, OutboundContextInternal,
        OutboundHandlerInternal,
    },
};

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
}

/// Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
/// over how an event is handled and how the Handlers in a pipeline interact with each other.
pub struct Pipeline<R, W> {
    handler_names: Vec<String>,

    inbound_handlers: Vec<Rc<RefCell<dyn InboundHandlerInternal>>>,
    inbound_contexts: Vec<Rc<RefCell<dyn InboundContextInternal>>>,

    outbound_handlers: Vec<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    outbound_contexts: Vec<Rc<RefCell<dyn OutboundContextInternal>>>,

    phantom: PhantomData<(R, W)>,
}

impl<R: 'static, W: 'static> Default for Pipeline<R, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: 'static, W: 'static> Pipeline<R, W> {
    /// Creates a new Pipeline
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

    /// Appends a [Handler] at the last position of this pipeline.
    fn add_back(&mut self, handler: impl Handler) {
        let (handler_name, inbound_handler, inbound_context, outbound_handler, outbound_context) =
            handler.generate();
        self.handler_names.push(handler_name);

        self.inbound_handlers.push(inbound_handler);
        self.inbound_contexts.push(inbound_context);

        self.outbound_handlers.push(outbound_handler);
        self.outbound_contexts.push(outbound_context);
    }

    /// Inserts a [Handler] at the first position of this pipeline.
    fn add_front(&mut self, handler: impl Handler) {
        let (handler_name, inbound_handler, inbound_context, outbound_handler, outbound_context) =
            handler.generate();
        self.handler_names.insert(0, handler_name);

        self.inbound_handlers.insert(0, inbound_handler);
        self.inbound_contexts.insert(0, inbound_context);

        self.outbound_handlers.insert(0, outbound_handler);
        self.outbound_contexts.insert(0, outbound_context);
    }

    /// Removes a [Handler] at the last position of this pipeline.
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

    /// Removes a [Handler] at the first position of this pipeline.
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

    /// Removes a [Handler] from this pipeline based on handler_name.
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

    #[allow(clippy::len_without_is_empty)]
    /// Returns the number of Handlers in this pipeline.
    fn len(&self) -> usize {
        self.handler_names.len()
    }

    /// Finalizes the pipeline.
    fn finalize(self) -> Self {
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

        self
    }
}

impl<R: 'static, W: 'static> InboundPipeline<R> for Pipeline<R, W> {
    /// Transport is active now, which means it is connected.
    fn transport_active(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.transport_active_internal(&*context);
    }

    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.transport_inactive_internal(&*context);
    }

    /// Reads a message.
    fn read(&self, msg: R) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_internal(&*context, Box::new(msg));
    }

    /// Reads an Error exception in one of its inbound operations.
    fn read_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_exception_internal(&*context, err);
    }

    /// Reads an EOF event.
    fn read_eof(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_eof_internal(&*context);
    }

    /// Handles a timeout event.
    fn handle_timeout(&self, now: Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.handle_timeout_internal(&*context, now);
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    fn poll_timeout(&self, eto: &mut Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.poll_timeout_internal(&*context, eto);
    }
}

impl<R: 'static, W: 'static> OutboundPipeline<W> for Pipeline<R, W> {
    /// Writes a message.
    fn write(&self, msg: W) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.write_internal(&*context, Box::new(msg));
    }

    /// Writes an Error exception from one of its outbound operations.
    fn write_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.write_exception_internal(&*context, err);
    }

    /// Writes a close event.
    fn close(&self) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.close_internal(&*context);
    }
}
