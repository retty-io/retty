//! The handler and pipeline APIs which are asynchronous and event-driven abstraction of various transports

#[cfg(not(feature = "metal-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "metal-io"))]
pub use self::async_io::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::Pipeline,
};

#[cfg(feature = "metal-io")]
pub(crate) mod metal_io;
#[cfg(feature = "metal-io")]
pub use metal_io::{
    handler::{Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler},
    handler_internal::{InboundHandlerInternal, OutboundHandlerInternal},
    pipeline::OutboundEvent,
    pipeline::Pipeline,
};
