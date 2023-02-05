use std::any::Any;
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::time::Instant;

#[doc(hidden)]
pub trait InboundHandlerInternal {
    fn transport_active_internal(&mut self, ctx: &dyn InboundContextInternal);
    fn transport_inactive_internal(&mut self, ctx: &dyn InboundContextInternal);

    fn read_internal(&mut self, ctx: &dyn InboundContextInternal, msg: Box<dyn Any>);
    fn read_exception_internal(&mut self, ctx: &dyn InboundContextInternal, err: Box<dyn Error>);
    fn read_eof_internal(&mut self, ctx: &dyn InboundContextInternal);

    fn read_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, now: Instant);
    fn poll_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, eto: &mut Instant);
}

#[doc(hidden)]
pub trait InboundContextInternal {
    fn fire_transport_active_internal(&self);
    fn fire_transport_inactive_internal(&self);
    fn fire_read_internal(&self, msg: Box<dyn Any>);
    fn fire_read_exception_internal(&self, err: Box<dyn Error>);
    fn fire_read_eof_internal(&self);
    fn fire_read_timeout_internal(&self, now: Instant);
    fn fire_poll_timeout_internal(&self, eto: &mut Instant);

    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
    fn set_next_in_context(
        &mut self,
        next_in_context: Option<Rc<RefCell<dyn InboundContextInternal>>>,
    );
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Rc<RefCell<dyn InboundHandlerInternal>>>,
    );
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    );
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    );
}

#[doc(hidden)]
pub trait OutboundHandlerInternal {
    fn write_internal(&mut self, ctx: &dyn OutboundContextInternal, msg: Box<dyn Any>);
    fn write_exception_internal(&mut self, ctx: &dyn OutboundContextInternal, err: Box<dyn Error>);
    fn close_internal(&mut self, ctx: &dyn OutboundContextInternal);
}

#[doc(hidden)]
pub trait OutboundContextInternal {
    fn fire_write_internal(&self, msg: Box<dyn Any>);
    fn fire_write_exception_internal(&self, err: Box<dyn Error>);
    fn fire_close_internal(&self);

    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    );
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    );
}
