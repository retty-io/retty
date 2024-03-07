//! The handler and pipeline APIs which are asynchronous and event-driven abstraction of various transports
pub(crate) mod handler;
pub(crate) mod handler_internal;
pub(crate) mod pipeline;
pub(crate) mod pipeline_internal;

pub use self::{
    handler::{Context, Handler},
    pipeline::{InboundPipeline, OutboundPipeline, Pipeline},
};
