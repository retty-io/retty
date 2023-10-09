//! The handler and pipeline APIs which are asynchronous and event-driven abstraction of various transports
pub(crate) mod handler;
pub(crate) mod handler_internal;
pub(crate) mod pipeline;
pub(crate) mod pipeline_internal;

#[cfg(test)]
pub(crate) mod channel_test;

pub use self::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{
        InboundContextInternal, InboundHandlerInternal, OutboundContextInternal,
        OutboundHandlerInternal,
    },
    pipeline::{AnyPipeline, InboundPipeline, OutboundPipeline, Pipeline},
};
