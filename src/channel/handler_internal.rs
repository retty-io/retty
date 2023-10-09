use std::{any::Any, cell::RefCell, error::Error, rc::Rc, time::Instant};

/// Internal Inbound Handler trait
pub trait InboundHandlerInternal {
    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn transport_active_internal(&mut self, ctx: &dyn InboundContextInternal);
    #[cfg(feature = "immutable")]
    fn transport_active_internal(&self, ctx: &dyn InboundContextInternal);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn transport_inactive_internal(&mut self, ctx: &dyn InboundContextInternal);
    #[cfg(feature = "immutable")]
    fn transport_inactive_internal(&self, ctx: &dyn InboundContextInternal);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn read_internal(&mut self, ctx: &dyn InboundContextInternal, msg: Box<dyn Any>);
    #[cfg(feature = "immutable")]
    fn read_internal(&self, ctx: &dyn InboundContextInternal, msg: Box<dyn Any>);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn read_exception_internal(&mut self, ctx: &dyn InboundContextInternal, err: Box<dyn Error>);
    #[cfg(feature = "immutable")]
    fn read_exception_internal(&self, ctx: &dyn InboundContextInternal, err: Box<dyn Error>);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn read_eof_internal(&mut self, ctx: &dyn InboundContextInternal);
    #[cfg(feature = "immutable")]
    fn read_eof_internal(&self, ctx: &dyn InboundContextInternal);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn handle_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, now: Instant);
    #[cfg(feature = "immutable")]
    fn handle_timeout_internal(&self, ctx: &dyn InboundContextInternal, now: Instant);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn poll_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, eto: &mut Instant);
    #[cfg(feature = "immutable")]
    fn poll_timeout_internal(&self, ctx: &dyn InboundContextInternal, eto: &mut Instant);

    /// Casts it to Any dyn trait
    fn as_any_internal(&self) -> &dyn Any;
}

/// Internal Inbound Context trait
pub trait InboundContextInternal {
    #[doc(hidden)]
    fn fire_transport_active_internal(&self);
    #[doc(hidden)]
    fn fire_transport_inactive_internal(&self);
    #[doc(hidden)]
    fn fire_read_internal(&self, msg: Box<dyn Any>);
    #[doc(hidden)]
    fn fire_read_exception_internal(&self, err: Box<dyn Error>);
    #[doc(hidden)]
    fn fire_read_eof_internal(&self);
    #[doc(hidden)]
    fn fire_handle_timeout_internal(&self, now: Instant);
    #[doc(hidden)]
    fn fire_poll_timeout_internal(&self, eto: &mut Instant);

    #[doc(hidden)]
    fn name(&self) -> &str;
    /// Casts it to Any dyn trait
    fn as_any_internal(&self) -> &dyn Any;
    #[doc(hidden)]
    fn set_next_in_context(
        &mut self,
        next_in_context: Option<Rc<RefCell<dyn InboundContextInternal>>>,
    );
    #[doc(hidden)]
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Rc<RefCell<dyn InboundHandlerInternal>>>,
    );
    #[doc(hidden)]
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    );
    #[doc(hidden)]
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    );
}

/// Internal Outbound Handler trait
pub trait OutboundHandlerInternal {
    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn write_internal(&mut self, ctx: &dyn OutboundContextInternal, msg: Box<dyn Any>);
    #[cfg(feature = "immutable")]
    fn write_internal(&self, ctx: &dyn OutboundContextInternal, msg: Box<dyn Any>);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn write_exception_internal(&mut self, ctx: &dyn OutboundContextInternal, err: Box<dyn Error>);
    #[cfg(feature = "immutable")]
    fn write_exception_internal(&self, ctx: &dyn OutboundContextInternal, err: Box<dyn Error>);

    #[doc(hidden)]
    #[cfg(not(feature = "immutable"))]
    fn close_internal(&mut self, ctx: &dyn OutboundContextInternal);
    #[cfg(feature = "immutable")]
    fn close_internal(&self, ctx: &dyn OutboundContextInternal);

    /// Casts it to Any dyn trait
    fn as_any_internal(&self) -> &dyn Any;
}

/// Internal Outbound Context trait
pub trait OutboundContextInternal {
    #[doc(hidden)]
    fn fire_write_internal(&self, msg: Box<dyn Any>);
    #[doc(hidden)]
    fn fire_write_exception_internal(&self, err: Box<dyn Error>);
    #[doc(hidden)]
    fn fire_close_internal(&self);

    #[doc(hidden)]
    fn name(&self) -> &str;
    /// Casts it to Any dyn trait
    fn as_any_internal(&self) -> &dyn Any;
    #[doc(hidden)]
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    );
    #[doc(hidden)]
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    );
}
