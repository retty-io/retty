use std::any::Any;
use std::sync::Arc;
use std::time::Instant;

use crate::channel::handler::Handler;
use crate::channel::handler_internal::{
    InboundHandlerContextInternal, InboundHandlerInternal, OutboundHandlerContextInternal,
    OutboundHandlerInternal,
};
use crate::error::Error;
use crate::runtime::sync::Mutex;
use crate::transport::TransportContext;

pub struct Pipeline {
    pub(crate) transport_ctx: TransportContext,
    pub(crate) inbound_contexts: Vec<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    pub(crate) inbound_handlers: Vec<Arc<Mutex<dyn InboundHandlerInternal>>>,
    pub(crate) outbound_contexts: Vec<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    pub(crate) outbound_handlers: Vec<Arc<Mutex<dyn OutboundHandlerInternal>>>,
}

impl Pipeline {
    pub fn new(transport_ctx: TransportContext) -> Self {
        Self {
            transport_ctx,
            inbound_contexts: Vec::new(),
            inbound_handlers: Vec::new(),
            outbound_contexts: Vec::new(),
            outbound_handlers: Vec::new(),
        }
    }
    pub fn add_back(&mut self, handler: impl Handler) {
        let (inbound_context, inbound_handler, outbound_context, outbound_handler) =
            handler.generate();
        self.inbound_contexts.push(inbound_context);
        self.inbound_handlers.push(inbound_handler);
        self.outbound_contexts.push(outbound_context);
        self.outbound_handlers.push(outbound_handler);
    }

    pub fn add_front(&mut self, handler: impl Handler) {
        let (inbound_context, inbound_handler, outbound_context, outbound_handler) =
            handler.generate();
        self.inbound_contexts.insert(0, inbound_context);
        self.inbound_handlers.insert(0, inbound_handler);
        self.outbound_contexts.insert(0, outbound_context);
        self.outbound_handlers.insert(0, outbound_handler);
    }

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

    fn header_handler_ctx(&self) -> Arc<Mutex<dyn InboundHandlerContextInternal>> {
        self.inbound_contexts.first().unwrap().clone()
    }

    fn header_handler(&self) -> Arc<Mutex<dyn InboundHandlerInternal>> {
        self.inbound_handlers.first().unwrap().clone()
    }

    fn tail_handler_ctx(&self) -> Arc<Mutex<dyn OutboundHandlerContextInternal>> {
        self.outbound_contexts.last().unwrap().clone()
    }

    fn tail_handler(&self) -> Arc<Mutex<dyn OutboundHandlerInternal>> {
        self.outbound_handlers.last().unwrap().clone()
    }

    pub async fn transport_active(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_active_internal(&mut *ctx).await;
    }

    pub async fn transport_inactive(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_inactive_internal(&mut *ctx).await;
    }

    pub async fn read(&self, msg: &mut (dyn Any + Send + Sync)) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_internal(&mut *ctx, msg).await;
    }

    pub async fn read_timeout(&self, timeout: Instant) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_timeout_internal(&mut *ctx, timeout).await;
    }

    pub async fn read_exception(&self, error: Error) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_exception_internal(&mut *ctx, error).await;
    }

    pub async fn read_eof(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_eof_internal(&mut *ctx).await;
    }

    pub async fn poll_timeout(&self, timeout: &mut Instant) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.poll_timeout_internal(&mut *ctx, timeout).await;
    }

    pub async fn write(&self, msg: &mut (dyn Any + Send + Sync)) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_internal(&mut *ctx, msg).await;
    }

    pub async fn write_exception(&self, error: Error) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_exception_internal(&mut *ctx, error).await;
    }

    pub async fn close(&self) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.close_internal(&mut *ctx).await;
    }
}
