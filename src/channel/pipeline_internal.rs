use std::{cell::RefCell, error::Error, io::ErrorKind, marker::PhantomData, rc::Rc, time::Instant};

use crate::channel::{
    handler::Handler,
    handler_internal::{
        InboundContextInternal, InboundHandlerInternal, OutboundContextInternal,
        OutboundHandlerInternal,
    },
};

pub(crate) struct PipelineInternal<R, W> {
    handler_names: Vec<String>,

    inbound_handlers: Vec<Rc<RefCell<dyn InboundHandlerInternal>>>,
    inbound_contexts: Vec<Rc<RefCell<dyn InboundContextInternal>>>,

    outbound_handlers: Vec<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    outbound_contexts: Vec<Rc<RefCell<dyn OutboundContextInternal>>>,

    phantom: PhantomData<(R, W)>,
}

impl<R: 'static, W: 'static> PipelineInternal<R, W> {
    pub(crate) fn new() -> Self {
        Self {
            handler_names: Vec::new(),

            inbound_handlers: Vec::new(),
            inbound_contexts: Vec::new(),

            outbound_handlers: Vec::new(),
            outbound_contexts: Vec::new(),

            phantom: PhantomData,
        }
    }

    pub(crate) fn add_back(&mut self, handler: impl Handler) -> Result<(), std::io::Error> {
        let (handler_name, inbound_handler, inbound_context, outbound_handler, outbound_context) =
            handler.generate();
        if self.handler_names.iter().any(|name| name == &handler_name) {
            Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("can't add_back exist handler with name {}", handler_name),
            ))
        } else {
            self.handler_names.push(handler_name);

            self.inbound_handlers.push(inbound_handler);
            self.inbound_contexts.push(inbound_context);

            self.outbound_handlers.push(outbound_handler);
            self.outbound_contexts.push(outbound_context);

            Ok(())
        }
    }

    pub(crate) fn add_front(&mut self, handler: impl Handler) -> Result<(), std::io::Error> {
        let (handler_name, inbound_handler, inbound_context, outbound_handler, outbound_context) =
            handler.generate();
        if self.handler_names.iter().any(|name| name == &handler_name) {
            Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("can't add_front exist handler with name {}", handler_name),
            ))
        } else {
            self.handler_names.insert(0, handler_name);

            self.inbound_handlers.insert(0, inbound_handler);
            self.inbound_contexts.insert(0, inbound_context);

            self.outbound_handlers.insert(0, outbound_handler);
            self.outbound_contexts.insert(0, outbound_context);

            Ok(())
        }
    }

    pub(crate) fn remove_back(&mut self) -> Result<(), std::io::Error> {
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

    pub(crate) fn remove_front(&mut self) -> Result<(), std::io::Error> {
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

    pub(crate) fn remove(&mut self, handler_name: &str) -> Result<(), std::io::Error> {
        for (index, name) in self.handler_names.iter().enumerate() {
            if name == handler_name {
                self.handler_names.remove(index);

                self.inbound_handlers.remove(index);
                self.inbound_contexts.remove(index);

                self.outbound_handlers.remove(index);
                self.outbound_contexts.remove(index);

                return Ok(());
            }
        }
        Err(std::io::Error::new(
            ErrorKind::NotFound,
            format!("No such handler \"{}\" in pipeline", handler_name),
        ))
    }

    pub(crate) fn get_inbound_handler(
        &self,
        handler_name: &str,
    ) -> Option<Rc<RefCell<dyn InboundHandlerInternal>>> {
        for (index, name) in self.handler_names.iter().enumerate() {
            if name == handler_name {
                return Some(self.inbound_handlers[index].clone());
            }
        }
        None
    }

    pub(crate) fn get_outbound_handler(
        &self,
        handler_name: &str,
    ) -> Option<Rc<RefCell<dyn OutboundHandlerInternal>>> {
        for (index, name) in self.handler_names.iter().enumerate() {
            if name == handler_name {
                return Some(self.outbound_handlers[index].clone());
            }
        }
        None
    }

    pub(crate) fn get_inbound_context(
        &self,
        handler_name: &str,
    ) -> Option<Rc<RefCell<dyn InboundContextInternal>>> {
        for (index, name) in self.handler_names.iter().enumerate() {
            if name == handler_name {
                return Some(self.inbound_contexts[index].clone());
            }
        }
        None
    }

    pub(crate) fn get_outbound_context(
        &self,
        handler_name: &str,
    ) -> Option<Rc<RefCell<dyn OutboundContextInternal>>> {
        for (index, name) in self.handler_names.iter().enumerate() {
            if name == handler_name {
                return Some(self.outbound_contexts[index].clone());
            }
        }
        None
    }

    pub(crate) fn len(&self) -> usize {
        self.handler_names.len()
    }

    pub(crate) fn finalize(&self) {
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

    pub(crate) fn transport_active(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.transport_active_internal(&*context);
    }

    pub(crate) fn transport_inactive(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.transport_inactive_internal(&*context);
    }

    pub(crate) fn read(&self, msg: R) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_internal(&*context, Box::new(msg));
    }

    pub(crate) fn read_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_exception_internal(&*context, err);
    }

    pub(crate) fn read_eof(&self) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.read_eof_internal(&*context);
    }

    pub(crate) fn handle_timeout(&self, now: Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.handle_timeout_internal(&*context, now);
    }

    pub(crate) fn poll_timeout(&self, eto: &mut Instant) {
        let (mut handler, context) = (
            self.inbound_handlers.first().unwrap().borrow_mut(),
            self.inbound_contexts.first().unwrap().borrow(),
        );
        handler.poll_timeout_internal(&*context, eto);
    }

    pub(crate) fn write(&self, msg: W) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.write_internal(&*context, Box::new(msg));
    }

    pub(crate) fn write_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.write_exception_internal(&*context, err);
    }

    pub(crate) fn close(&self) {
        let (mut handler, context) = (
            self.outbound_handlers.last().unwrap().borrow_mut(),
            self.outbound_contexts.last().unwrap().borrow(),
        );
        handler.close_internal(&*context);
    }
}
