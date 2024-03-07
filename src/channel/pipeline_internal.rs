use std::collections::VecDeque;
use std::{cell::RefCell, error::Error, io::ErrorKind, marker::PhantomData, rc::Rc, time::Instant};

use crate::channel::{
    handler::Handler,
    handler_internal::{ContextInternal, HandlerInternal},
    Context,
};

const RESERVED_RETTY_PIPELINE_HANDLE_NAME: &str = "ReservedRettyPipelineHandlerName";

pub(crate) struct PipelineInternal<R, W> {
    names: Vec<String>,
    handlers: Vec<Rc<RefCell<dyn HandlerInternal>>>,
    contexts: Vec<Rc<RefCell<dyn ContextInternal>>>,

    transmits: Rc<RefCell<VecDeque<W>>>,
    phantom: PhantomData<R>,
}

impl<R: 'static, W: 'static> PipelineInternal<R, W> {
    pub(crate) fn new() -> Self {
        let transmits = Rc::new(RefCell::new(VecDeque::new()));
        let last_handler = LastHandler::new(transmits.clone());
        let (name, handler, context) = last_handler.generate();
        Self {
            names: vec![name],
            handlers: vec![handler],
            contexts: vec![context],

            transmits,
            phantom: PhantomData,
        }
    }

    pub(crate) fn add_back(&mut self, handler: impl Handler + 'static) {
        let (name, handler, context) = handler.generate();
        if name == RESERVED_RETTY_PIPELINE_HANDLE_NAME {
            panic!("handle name {} is reserved", name);
        }

        let len = self.names.len();

        self.names.insert(len - 1, name);
        self.handlers.insert(len - 1, handler);
        self.contexts.insert(len - 1, context);
    }

    pub(crate) fn add_front(&mut self, handler: impl Handler + 'static) {
        let (name, handler, context) = handler.generate();
        if name == RESERVED_RETTY_PIPELINE_HANDLE_NAME {
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
        if handler_name == RESERVED_RETTY_PIPELINE_HANDLE_NAME {
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

    pub(crate) fn finalize(&self) {
        let mut enumerate = self.contexts.iter().enumerate();
        let ctx_pipe_len = self.contexts.len();
        for _ in 0..ctx_pipe_len {
            let (j, ctx) = enumerate.next().unwrap();
            let mut curr = ctx.borrow_mut();

            let (next_context, next_handler) = (self.contexts.get(j + 1), self.handlers.get(j + 1));
            match (next_context, next_handler) {
                (Some(next_ctx), Some(next_hdr)) => {
                    curr.set_next_context(Some(next_ctx.clone()));
                    curr.set_next_handler(Some(next_hdr.clone()));
                }
                _ => {
                    curr.set_next_context(None);
                    curr.set_next_handler(None);
                }
            }
        }
    }

    pub(crate) fn write(&self, msg: W) {
        let mut transmits = self.transmits.borrow_mut();
        transmits.push_back(msg);
    }

    pub(crate) fn transport_active(&self) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.transport_active_internal(&*context);
    }

    pub(crate) fn transport_inactive(&self) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.transport_inactive_internal(&*context);
    }

    pub(crate) fn handle_read(&self, msg: R) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.handle_read_internal(&*context, Box::new(msg));
    }

    pub(crate) fn poll_write(&self) -> Option<R> {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        if let Some(msg) = handler.poll_write_internal(&*context) {
            if let Ok(msg) = msg.downcast::<R>() {
                Some(*msg)
            } else {
                panic!("msg can't downcast::<R> in {} handler", context.name());
            }
        } else {
            None
        }
    }

    pub(crate) fn handle_close(&self) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.handle_close_internal(&*context);
    }

    pub(crate) fn handle_timeout(&self, now: Instant) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.handle_timeout_internal(&*context, now);
    }

    pub(crate) fn poll_timeout(&self, eto: &mut Instant) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.poll_timeout_internal(&*context, eto);
    }

    pub(crate) fn handle_read_eof(&self) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.handle_read_eof_internal(&*context);
    }

    pub(crate) fn handle_exception(&self, err: Box<dyn Error>) {
        let (mut handler, context) = (
            self.handlers.first().unwrap().borrow_mut(),
            self.contexts.first().unwrap().borrow(),
        );
        handler.handle_exception_internal(&*context, err);
    }
}

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
        RESERVED_RETTY_PIPELINE_HANDLE_NAME
    }

    fn handle_read(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    ) {
        ctx.fire_read(msg);
    }

    fn poll_write(
        &mut self,
        _ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout> {
        let mut transmits = self.transmits.borrow_mut();
        transmits.pop_front()
    }
}
