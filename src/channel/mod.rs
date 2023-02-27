//! The handler and pipeline APIs which are asynchronous and event-driven abstraction of various transports

#[cfg(not(feature = "sync-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "sync-io"))]
pub use self::async_io::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::Pipeline,
};

#[cfg(feature = "sync-io")]
pub(crate) mod sync_io;
#[cfg(feature = "sync-io")]
pub use sync_io::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::OutboundEvent,
    pipeline::Pipeline,
};
