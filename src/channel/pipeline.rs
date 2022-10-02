use std::sync::Arc;

use crate::channel::handler::{
    Handler, InboundHandler, InboundHandlerContext, OutboundHandler, OutboundHandlerContext,
};
use crate::error::Error;
use crate::runtime::sync::Mutex;
use crate::Message;

pub struct Pipeline {
    pub(crate) inbound_handlers: Vec<Arc<Mutex<dyn InboundHandler>>>,
    pub(crate) outbound_handlers: Vec<Arc<Mutex<dyn OutboundHandler>>>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            inbound_handlers: Vec::new(),
            outbound_handlers: Vec::new(),
        }
    }
    pub fn add_back(&mut self, handler: impl Handler) {
        let (inbound, outbound) = handler.split();
        self.inbound_handlers.push(inbound);
        self.outbound_handlers.push(outbound);
    }

    pub fn add_front(&mut self, handler: impl Handler) {
        let (inbound, outbound) = handler.split();
        self.inbound_handlers.insert(0, inbound);
        self.outbound_handlers.insert(0, outbound);
    }

    pub async fn finalize(&mut self) -> PipelineContext {
        let mut pipeline_context = PipelineContext::new();

        let (inbound_handlers, outbound_handlers) = (
            self.inbound_handlers.drain(..),
            self.outbound_handlers.drain(..),
        );
        for (inbound_handler, outbound_handler) in inbound_handlers
            .into_iter()
            .zip(outbound_handlers.into_iter())
        {
            let inbound_context = Arc::new(Mutex::new(InboundHandlerContext::default()));
            let outbound_context = Arc::new(Mutex::new(OutboundHandlerContext::default()));

            pipeline_context.add_back(
                inbound_context,
                inbound_handler,
                outbound_context,
                outbound_handler,
            );
        }

        let mut enumerate = pipeline_context.inbound_contexts.iter().enumerate();
        let ctx_pipe_len = pipeline_context.inbound_contexts.len();
        for _ in 0..ctx_pipe_len {
            let (j, ctx) = enumerate.next().unwrap();
            let mut curr = ctx.lock().await;

            {
                let (next_context, next_handler) = (
                    pipeline_context.inbound_contexts.get(j + 1),
                    pipeline_context.inbound_handlers.get(j + 1),
                );
                match (next_context, next_handler) {
                    (Some(next_ctx), Some(next_hdlr)) => {
                        curr.next_in_ctx = Some(next_ctx.clone());
                        curr.next_in_handler = Some(next_hdlr.clone());
                    }
                    _ => {
                        curr.next_in_ctx = None;
                        curr.next_in_handler = None;
                    }
                }
            }

            {
                let (prev_context, prev_handler) = if j > 0 {
                    (
                        pipeline_context.outbound_contexts.get(j - 1),
                        pipeline_context.outbound_handlers.get(j - 1),
                    )
                } else {
                    (None, None)
                };
                match (prev_context, prev_handler) {
                    (Some(prev_ctx), Some(prev_hdlr)) => {
                        curr.next_out_ctx = Some(prev_ctx.clone());
                        curr.next_out_handler = Some(prev_hdlr.clone());
                    }
                    _ => {
                        curr.next_out_ctx = None;
                        curr.next_out_handler = None;
                    }
                }
            }
        }

        let mut enumerate = pipeline_context.outbound_contexts.iter().enumerate();
        let ctx_pipe_len = pipeline_context.outbound_contexts.len();
        for _ in 0..ctx_pipe_len {
            let (j, ctx) = enumerate.next().unwrap();
            let mut curr = ctx.lock().await;

            {
                let (prev_context, prev_handler) = if j > 0 {
                    (
                        pipeline_context.outbound_contexts.get(j - 1),
                        pipeline_context.outbound_handlers.get(j - 1),
                    )
                } else {
                    (None, None)
                };
                match (prev_context, prev_handler) {
                    (Some(prev_ctx), Some(prev_hdlr)) => {
                        curr.next_out_ctx = Some(prev_ctx.clone());
                        curr.next_out_handler = Some(prev_hdlr.clone());
                    }
                    _ => {
                        curr.next_out_ctx = None;
                        curr.next_out_handler = None;
                    }
                }
            }
        }
        pipeline_context
    }
}

#[derive(Clone)]
pub struct PipelineContext {
    pub(crate) inbound_contexts: Vec<Arc<Mutex<InboundHandlerContext>>>,
    pub(crate) inbound_handlers: Vec<Arc<Mutex<dyn InboundHandler>>>,

    pub(crate) outbound_contexts: Vec<Arc<Mutex<OutboundHandlerContext>>>,
    pub(crate) outbound_handlers: Vec<Arc<Mutex<dyn OutboundHandler>>>,
}

impl PipelineContext {
    pub(crate) fn new() -> PipelineContext {
        PipelineContext {
            inbound_contexts: vec![],
            inbound_handlers: vec![],

            outbound_contexts: vec![],
            outbound_handlers: vec![],
        }
    }

    pub(crate) fn add_back(
        &mut self,
        inbound_context: Arc<Mutex<InboundHandlerContext>>,
        inbound_handler: Arc<Mutex<dyn InboundHandler>>,
        outbound_context: Arc<Mutex<OutboundHandlerContext>>,
        outbound_handler: Arc<Mutex<dyn OutboundHandler>>,
    ) {
        self.inbound_contexts.push(inbound_context);
        self.inbound_handlers.push(inbound_handler);

        self.outbound_contexts.push(outbound_context);
        self.outbound_handlers.push(outbound_handler);
    }

    fn header_handler_ctx(&self) -> Arc<Mutex<InboundHandlerContext>> {
        self.inbound_contexts.first().unwrap().clone()
    }

    fn header_handler(&self) -> Arc<Mutex<dyn InboundHandler>> {
        self.inbound_handlers.first().unwrap().clone()
    }

    fn tail_handler_ctx(&self) -> Arc<Mutex<OutboundHandlerContext>> {
        self.outbound_contexts.last().unwrap().clone()
    }

    fn tail_handler(&self) -> Arc<Mutex<dyn OutboundHandler>> {
        self.outbound_handlers.last().unwrap().clone()
    }

    pub async fn transport_active(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_active(&mut ctx).await;
    }

    pub async fn transport_inactive(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.transport_inactive(&mut ctx).await;
    }

    pub async fn read(&self, msg: Message) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read(&mut ctx, msg).await;
    }

    pub async fn read_exception(&self, error: Error) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_exception(&mut ctx, error).await;
    }

    pub async fn read_eof(&self) {
        let (mut handler, mut ctx) = (
            self.inbound_handlers.first().unwrap().lock().await,
            self.inbound_contexts.first().unwrap().lock().await,
        );
        handler.read_eof(&mut ctx).await;
    }

    pub async fn write(&self, msg: Message) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write(&mut ctx, msg).await;
    }

    pub async fn write_exception(&self, error: Error) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.write_exception(&mut ctx, error).await;
    }

    pub async fn close(&self) {
        let (mut handler, mut ctx) = (
            self.outbound_handlers.last().unwrap().lock().await,
            self.outbound_contexts.last().unwrap().lock().await,
        );
        handler.close(&mut ctx).await;
    }
}
