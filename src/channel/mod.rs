//! The handler and pipeline APIs which are asynchronous and event-driven abstraction of various transports

#[cfg(not(feature = "sans-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "sans-io"))]
pub use self::async_io::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::Pipeline,
};

#[cfg(feature = "sans-io")]
pub(crate) mod sans_io;
#[cfg(feature = "sans-io")]
pub use sans_io::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::Pipeline,
};
