use std::any::Any;
use std::error::Error;
use std::time::Instant;

#[doc(hidden)]
pub trait HandlerInternal {
    fn transport_active_internal(&mut self, ctx: &dyn ContextInternal);
    fn transport_inactive_internal(&mut self, ctx: &dyn ContextInternal);

    fn handle_read_internal(&mut self, ctx: &dyn ContextInternal, msg: Box<dyn Any>);
    fn poll_write_internal(&mut self, ctx: &dyn ContextInternal) -> Option<Box<dyn Any>>;

    fn handle_timeout_internal(&mut self, ctx: &dyn ContextInternal, now: Instant);
    fn poll_timeout_internal(&mut self, ctx: &dyn ContextInternal, eto: &mut Instant);

    fn handle_eof_internal(&mut self, ctx: &dyn ContextInternal);
    fn handle_error_internal(&mut self, ctx: &dyn ContextInternal, err: Box<dyn Error>);
    fn handle_close_internal(&mut self, ctx: &dyn ContextInternal);

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[doc(hidden)]
pub trait ContextInternal {
    fn fire_transport_active_internal(&self);
    fn fire_transport_inactive_internal(&self);

    fn fire_handle_read_internal(&self, msg: Box<dyn Any>);
    fn fire_poll_write_internal(&self) -> Option<Box<dyn Any>>;

    fn fire_handle_timeout_internal(&self, now: Instant);
    fn fire_poll_timeout_internal(&self, eto: &mut Instant);

    fn fire_handle_eof_internal(&self);
    fn fire_handle_error_internal(&self, err: Box<dyn Error>);
    fn fire_handle_close_internal(&self);

    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
    fn set_next_context(&mut self, next_context: *mut dyn ContextInternal);
    fn set_next_handler(&mut self, next_handler: *mut dyn HandlerInternal);
    fn clear_next(&mut self);
}
