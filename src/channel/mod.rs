//! The handler and pipeline APIs which are asynchronous and event-driven abstraction of various transports
#[cfg(test)]
pub(crate) mod channel_test;

#[cfg(not(feature = "sans-io"))]
mod async_channel;
#[cfg(not(feature = "sans-io"))]
pub use async_channel::handler::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler,
};
#[cfg(not(feature = "sans-io"))]
pub use async_channel::handler_internal::{InboundHandlerInternal, OutboundHandlerInternal};
#[cfg(not(feature = "sans-io"))]
pub use async_channel::pipeline::Pipeline;

#[cfg(feature = "sans-io")]
mod sansio_channel;
#[cfg(feature = "sans-io")]
pub use sansio_channel::handler::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler,
};
#[cfg(feature = "sans-io")]
pub use sansio_channel::handler_internal::{InboundHandlerInternal, OutboundHandlerInternal};
#[cfg(feature = "sans-io")]
pub use sansio_channel::pipeline::Pipeline;
