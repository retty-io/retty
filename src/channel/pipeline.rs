use std::error::Error;
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

/// Pipeline implements an advanced form of the Intercepting Filter pattern to give a user full control
/// over how an event is handled and how the Handlers in a pipeline interact with each other.
pub struct Pipeline<R, W> {
    inbound_contexts: Vec<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    inbound_handlers: Vec<Arc<Mutex<dyn InboundHandlerInternal>>>,
    outbound_contexts: Vec<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    outbound_handlers: Vec<Arc<Mutex<dyn OutboundHandlerInternal>>>,

    phantom_r: PhantomData<R>,
    phantom_w: PhantomData<W>,
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
            inbound_contexts: Vec::new(),
            inbound_handlers: Vec::new(),
            outbound_contexts: Vec::new(),
            outbound_handlers: Vec::new(),

            phantom_r: PhantomData,
            phantom_w: PhantomData,
        }
    }

    /// Appends a [Handler] at the last position of this pipeline.
    pub fn add_back(&mut self, handler: impl Handler) {
        let (inbound_context, inbound_handler, outbound_context, outbound_handler) =
            handler.generate();
        self.inbound_contexts.push(inbound_context);
        self.inbound_handlers.push(inbound_handler);
        self.outbound_contexts.push(outbound_context);
        self.outbound_handlers.push(outbound_handler);
    }

    /// Inserts a [Handler] at the first position of this pipeline.
    pub fn add_front(&mut self, handler: impl Handler) {
        let (inbound_context, inbound_handler, outbound_context, outbound_handler) =
            handler.generate();
        self.inbound_contexts.insert(0, inbound_context);
        self.inbound_handlers.insert(0, inbound_handler);
        self.outbound_contexts.insert(0, outbound_context);
        self.outbound_handlers.insert(0, outbound_handler);
    }

    /// Finalizes the pipeline
    pub async fn finalize(self) -> Self {
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
                        curr.set_next_in_ctx(Some(next_ctx.clone()));
                        curr.set_next_in_handler(Some(next_hdlr.clone()));
                    }
                    _ => {
                        curr.set_next_in_ctx(None);
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
                        curr.set_next_out_ctx(Some(prev_ctx.clone()));
                        curr.set_next_out_handler(Some(prev_hdlr.clone()));
                    }
                    _ => {
                        curr.set_next_out_ctx(None);
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
                        curr.set_next_out_ctx(Some(prev_ctx.clone()));
                        curr.set_next_out_handler(Some(prev_hdlr.clone()));
                    }
                    _ => {
                        curr.set_next_out_ctx(None);
                        curr.set_next_out_handler(None);
                    }
                }
            }
        }

        self
    }

    /// Transport is active now, which means it is connected.
    pub async fn transport_active(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_active_internal(&mut *ctx).await;
    }

    /// Transport is inactive now, which means it is disconnected.
    pub async fn transport_inactive(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_inactive_internal(&mut *ctx).await;
    }

    /// Reads a message.
    pub async fn read(&self, msg: R) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_internal(&mut *ctx, Box::new(msg)).await;
    }

    /// Reads an Error exception in one of its inbound operations.
    pub async fn read_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_exception_internal(&mut *ctx, err).await;
    }

    /// Reads an EOF event.
    pub async fn read_eof(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_eof_internal(&mut *ctx).await;
    }

    /// Reads a timeout event.
    pub async fn read_timeout(&self, timeout: Instant) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_timeout_internal(&mut *ctx, timeout).await;
    }

    /// Polls timout event in its inbound operations.
    /// If any inbound handler has timeout event to trigger in future,
    /// it should compare its own timeout event with the provided timout event and
    /// update the provided timeout event with the minimal of these two timeout events.
    pub async fn poll_timeout(&self, timeout: &mut Instant) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.poll_timeout_internal(&mut *ctx, timeout).await;
    }

    /// Writes a message.
    pub async fn write(&self, msg: W) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_internal(&mut *ctx, Box::new(msg)).await;
    }

    /// Writes an Error exception from one of its outbound operations.
    pub async fn write_exception(&self, err: Box<dyn Error + Send + Sync>) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_exception_internal(&mut *ctx, err).await;
    }

    /// Writes a close event.
    pub async fn close(&self) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.close_internal(&mut *ctx).await;
    }
}
