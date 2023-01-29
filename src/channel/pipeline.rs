use std::error::Error;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use crate::channel::{
    handler::Handler,
    handler_internal::{
        InboundHandlerContextInternal, InboundHandlerInternal, OutboundHandlerContextInternal,
        OutboundHandlerInternal,
    },
};
use crate::runtime::sync::Mutex;

struct PipelineInternal<R, W> {
    handler_names: Vec<String>,
    inbound_contexts: Vec<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    inbound_handlers: Vec<Arc<Mutex<dyn InboundHandlerInternal>>>,
    outbound_contexts: Vec<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    outbound_handlers: Vec<Arc<Mutex<dyn OutboundHandlerInternal>>>,

    phantom_r: PhantomData<R>,
    phantom_w: PhantomData<W>,
}

impl<R: Send + Sync + 'static, W: Send + Sync + 'static> PipelineInternal<R, W> {
    fn new() -> Self {
        Self {
            handler_names: Vec::new(),
            inbound_contexts: Vec::new(),
            inbound_handlers: Vec::new(),
            outbound_contexts: Vec::new(),
            outbound_handlers: Vec::new(),

            phantom_r: PhantomData,
            phantom_w: PhantomData,
        }
    }

    fn add_back(&mut self, handler: impl Handler) {
        let (handler_name, inbound_context, inbound_handler, outbound_context, outbound_handler) =
            handler.generate();
        self.handler_names.push(handler_name);
        self.inbound_contexts.push(inbound_context);
        self.inbound_handlers.push(inbound_handler);
        self.outbound_contexts.push(outbound_context);
        self.outbound_handlers.push(outbound_handler);
    }

    fn add_front(&mut self, handler: impl Handler) {
        let (handler_name, inbound_context, inbound_handler, outbound_context, outbound_handler) =
            handler.generate();
        self.handler_names.insert(0, handler_name);
        self.inbound_contexts.insert(0, inbound_context);
        self.inbound_handlers.insert(0, inbound_handler);
        self.outbound_contexts.insert(0, outbound_context);
        self.outbound_handlers.insert(0, outbound_handler);
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
            self.inbound_contexts.remove(len - 1);
            self.inbound_handlers.remove(len - 1);
            self.outbound_contexts.remove(len - 1);
            self.outbound_handlers.remove(len - 1);

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
            self.inbound_contexts.remove(0);
            self.inbound_handlers.remove(0);
            self.outbound_contexts.remove(0);
            self.outbound_handlers.remove(0);

            Ok(())
        }
    }

    fn remove(&mut self, handler_name: &str) -> Result<(), std::io::Error> {
        let mut to_be_removed_in_reverse_order = vec![];
        for (index, name) in self.handler_names.iter().rev().enumerate() {
            if name == handler_name {
                to_be_removed_in_reverse_order.push(index);
            }
        }

        if !to_be_removed_in_reverse_order.is_empty() {
            for index in to_be_removed_in_reverse_order {
                self.handler_names.remove(index);
                self.inbound_contexts.remove(index);
                self.inbound_handlers.remove(index);
                self.outbound_contexts.remove(index);
                self.outbound_handlers.remove(index);
            }

            Ok(())
        } else {
            Err(std::io::Error::new(
                ErrorKind::NotFound,
                "No such handler in pipeline",
            ))
        }
    }

    async fn finalize(&self) {
        let mut enumerate = self.inbound_contexts.iter().enumerate();
        let ctx_pipe_len = self.inbound_contexts.len();
        for _ in 0..ctx_pipe_len {
            let (j, ctx) = enumerate.next().unwrap();
            let mut curr = ctx.lock().await;

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
            let mut curr = ctx.lock().await;

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

    async fn transport_active(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_active_internal(&mut *ctx).await;
    }

    async fn transport_inactive(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_inactive_internal(&mut *ctx).await;
    }

    async fn read(&self, msg: R) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_internal(&mut *ctx, Box::new(msg)).await;
    }

    async fn read_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_exception_internal(&mut *ctx, err).await;
    }

    async fn read_eof(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_eof_internal(&mut *ctx).await;
    }

    async fn read_timeout(&self, timeout: Instant) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_timeout_internal(&mut *ctx, timeout).await;
    }

    async fn poll_timeout(&self, timeout: &mut Instant) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.poll_timeout_internal(&mut *ctx, timeout).await;
    }

    async fn write(&self, msg: W) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_internal(&mut *ctx, Box::new(msg)).await;
    }

    async fn write_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_exception_internal(&mut *ctx, err).await;
    }

    async fn close(&self) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.close_internal(&mut *ctx).await;
    }
}

/// Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
/// over how an event is handled and how the Handlers in a pipeline interact with each other.
pub struct Pipeline<R, W> {
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
            internal: Mutex::new(PipelineInternal::new()),
        }
    }

    /// Appends a [Handler] at the last position of this pipeline.
    pub async fn add_back(&self, handler: impl Handler) {
        let mut internal = self.internal.lock().await;
        internal.add_back(handler);
    }

    /// Inserts a [Handler] at the first position of this pipeline.
    pub async fn add_front(&self, handler: impl Handler) {
        let mut internal = self.internal.lock().await;
        internal.add_front(handler);
    }

    /// Removes a [Handler] at the last position of this pipeline.
    pub async fn remove_back(&self) -> Result<(), std::io::Error> {
        let mut internal = self.internal.lock().await;
        internal.remove_back()
    }

    /// Removes a [Handler] at the first position of this pipeline.
    pub async fn remove_front(&self) -> Result<(), std::io::Error> {
        let mut internal = self.internal.lock().await;
        internal.remove_front()
    }

    /// Remove a [Handler] from this pipeline based on handler_name.
    pub async fn remove(&self, handler_name: &str) -> Result<(), std::io::Error> {
        let mut internal = self.internal.lock().await;
        internal.remove(handler_name)
    }

    /// Finalizes the pipeline
    pub async fn finalize(&self) {
        let internal = self.internal.lock().await;
        internal.finalize().await;
    }

    /// Transport is active now, which means it is connected.
    pub async fn transport_active(&self) {
        let internal = self.internal.lock().await;
        internal.transport_active().await;
    }

    /// Transport is inactive now, which means it is disconnected.
    pub async fn transport_inactive(&self) {
        let internal = self.internal.lock().await;
        internal.transport_inactive().await;
    }

    /// Reads a message.
    pub async fn read(&self, msg: R) {
        let internal = self.internal.lock().await;
        internal.read(msg).await;
    }

    /// Reads an Error exception in one of its inbound operations.
    pub async fn read_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let internal = self.internal.lock().await;
        internal.read_exception(err).await;
    }

    /// Reads an EOF event.
    pub async fn read_eof(&self) {
        let internal = self.internal.lock().await;
        internal.read_eof().await;
    }

    /// Reads a timeout event.
    pub async fn read_timeout(&self, timeout: Instant) {
        let internal = self.internal.lock().await;
        internal.read_timeout(timeout).await;
    }

    /// Polls timout event in its inbound operations.
    /// If any inbound handler has timeout event to trigger in future,
    /// it should compare its own timeout event with the provided timout event and
    /// update the provided timeout event with the minimal of these two timeout events.
    pub async fn poll_timeout(&self, timeout: &mut Instant) {
        let internal = self.internal.lock().await;
        internal.poll_timeout(timeout).await;
    }

    /// Writes a message.
    pub async fn write(&self, msg: W) {
        let internal = self.internal.lock().await;
        internal.write(msg).await;
    }

    /// Writes an Error exception from one of its outbound operations.
    pub async fn write_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let internal = self.internal.lock().await;
        internal.write_exception(err).await;
    }

    /// Writes a close event.
    pub async fn close(&self) {
        let internal = self.internal.lock().await;
        internal.close().await;
    }
}
