use crate::channel::handler_internal::{ContextInternal, HandlerInternal};
use log::{trace, warn};
use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::{error::Error, time::Instant};

/// Handles both inbound and outbound events
pub trait Handler {
    /// Associated read input message type
    type Rin: 'static;
    /// Associated read output message type
    type Rout: 'static;
    /// Associated write input message type
    type Win: 'static;
    /// Associated write output message type for
    type Wout: 'static;

    /// Returns handler name
    fn name(&self) -> &str;

    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    fn generate(
        self,
    ) -> (
        String,
        Rc<RefCell<dyn HandlerInternal>>,
        Rc<RefCell<dyn ContextInternal>>,
    )
    where
        Self: Sized + 'static,
    {
        let handler_name = self.name().to_owned();
        let context: Context<Self::Rin, Self::Rout, Self::Win, Self::Wout> =
            Context::new(self.name());

        let handler: Box<
            dyn Handler<Rin = Self::Rin, Rout = Self::Rout, Win = Self::Win, Wout = Self::Wout>,
        > = Box::new(self);

        (
            handler_name,
            Rc::new(RefCell::new(handler)),
            Rc::new(RefCell::new(context)),
        )
    }

    /// Transport is active now, which means it is connected.
    fn transport_active(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_transport_active();
    }
    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_transport_inactive();
    }

    /// Handles input message.
    fn handle_read(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        msg: Self::Rin,
    );
    /// Polls output message from internal transmit queue
    fn poll_write(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
    ) -> Option<Self::Wout>;

    /// Handles a timeout event.
    fn handle_timeout(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        now: Instant,
    ) {
        ctx.fire_timeout(now);
    }
    /// Polls earliest timeout (eto) in its inbound operations.
    fn poll_timeout(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        eto: &mut Instant,
    ) {
        ctx.fire_poll_timeout(eto);
    }

    /// Reads an EOF event.
    fn handle_read_eof(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_read_eof();
    }
    /// Handle an Error exception in one of its operations.
    fn handle_exception(
        &mut self,
        ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>,
        err: Box<dyn Error>,
    ) {
        ctx.fire_exception(err);
    }
    /// Handle a close event.
    fn handle_close(&mut self, ctx: &Context<Self::Rin, Self::Rout, Self::Win, Self::Wout>) {
        ctx.fire_close();
    }
}

impl<Rin: 'static, Rout: 'static, Win: 'static, Wout: 'static> HandlerInternal
    for Box<dyn Handler<Rin = Rin, Rout = Rout, Win = Win, Wout = Wout>>
{
    fn transport_active_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.transport_active(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn transport_inactive_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.transport_inactive(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn handle_read_internal(&mut self, ctx: &dyn ContextInternal, msg: Box<dyn Any>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            if let Ok(msg) = msg.downcast::<Rin>() {
                self.handle_read(ctx, *msg);
            } else {
                panic!("msg can't downcast::<Rin> in {} handler", ctx.name());
            }
        } else {
            panic!(
                "ctx can't downcast::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn poll_write_internal(&mut self, ctx: &dyn ContextInternal) -> Option<Box<dyn Any>> {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            if let Some(msg) = self.poll_write(ctx) {
                Some(Box::new(msg))
            } else {
                None
            }
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn handle_timeout_internal(&mut self, ctx: &dyn ContextInternal, now: Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_timeout(ctx, now);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn poll_timeout_internal(&mut self, ctx: &dyn ContextInternal, eto: &mut Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.poll_timeout(ctx, eto);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn handle_read_eof_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_read_eof(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn handle_exception_internal(&mut self, ctx: &dyn ContextInternal, err: Box<dyn Error>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_exception(ctx, err);
        } else {
            panic!(
                "ctx can't downcast_ref::<Context<Rin, Rout, Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn handle_close_internal(&mut self, ctx: &dyn ContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<Context<Rin, Rout, Win, Wout>>() {
            self.handle_close(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
}

/// Enables a [Handler] to interact with its Pipeline and other handlers.
pub struct Context<Rin, Rout, Win, Wout> {
    name: String,

    next_context: Option<Rc<RefCell<dyn ContextInternal>>>,
    next_handler: Option<Rc<RefCell<dyn HandlerInternal>>>,

    phantom: PhantomData<(Rin, Rout, Win, Wout)>,
}

impl<Rin: 'static, Rout: 'static, Win: 'static, Wout: 'static> Context<Rin, Rout, Win, Wout> {
    /// Creates a new Context
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),

            next_context: None,
            next_handler: None,

            phantom: PhantomData,
        }
    }

    /// Transport is active now, which means it is connected.
    pub fn fire_transport_active(&self) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.transport_active_internal(&*next_context);
        }
    }

    /// Transport is inactive now, which means it is disconnected.
    pub fn fire_transport_inactive(&self) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.transport_inactive_internal(&*next_context);
        }
    }

    /// Handle input message.
    pub fn fire_read(&self, msg: Rout) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.handle_read_internal(&*next_context, Box::new(msg));
        } else {
            warn!("handle_read reached end of pipeline");
        }
    }

    /// Polls output message.
    pub fn fire_poll_write(&self) -> Option<Win> {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            if let Some(msg) = next_handler.poll_write_internal(&*next_context) {
                if let Ok(msg) = msg.downcast::<Win>() {
                    Some(*msg)
                } else {
                    panic!(
                        "msg can't downcast::<Win> in {} handler",
                        next_context.name()
                    );
                }
            } else {
                None
            }
        } else {
            warn!("poll_write reached end of pipeline");
            None
        }
    }

    /// Handles a timeout event.
    pub fn fire_timeout(&self, now: Instant) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.handle_timeout_internal(&*next_context, now);
        } else {
            warn!("handle_timeout reached end of pipeline");
        }
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    pub fn fire_poll_timeout(&self, eto: &mut Instant) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.poll_timeout_internal(&*next_context, eto);
        } else {
            trace!("poll_timeout reached end of pipeline");
        }
    }

    /// Reads an EOF event.
    pub fn fire_read_eof(&self) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.handle_read_eof_internal(&*next_context);
        } else {
            warn!("handle_read_eof reached end of pipeline");
        }
    }

    /// Reads an Error exception in one of its inbound operations.
    pub fn fire_exception(&self, err: Box<dyn Error>) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.handle_exception_internal(&*next_context, err);
        } else {
            warn!("handle_exception reached end of pipeline");
        }
    }

    /// Writes a close event.
    pub fn fire_close(&self) {
        if let (Some(next_handler), Some(next_context)) = (&self.next_handler, &self.next_context) {
            let (mut next_handler, next_context) =
                (next_handler.borrow_mut(), next_context.borrow());
            next_handler.handle_close_internal(&*next_context);
        } else {
            warn!("handle_close reached end of pipeline");
        }
    }
}

impl<Rin: 'static, Rout: 'static, Win: 'static, Wout: 'static> ContextInternal
    for Context<Rin, Rout, Win, Wout>
{
    fn fire_transport_active_internal(&self) {
        self.fire_transport_active();
    }
    fn fire_transport_inactive_internal(&self) {
        self.fire_transport_inactive();
    }
    fn fire_read_internal(&self, msg: Box<dyn Any>) {
        if let Ok(msg) = msg.downcast::<Rout>() {
            self.fire_read(*msg);
        } else {
            panic!("msg can't downcast::<Rout> in {} handler", self.name());
        }
    }
    fn fire_poll_write_internal(&self) -> Option<Box<dyn Any>> {
        if let Some(msg) = self.fire_poll_write() {
            Some(Box::new(msg))
        } else {
            None
        }
    }

    fn fire_timeout_internal(&self, now: Instant) {
        self.fire_timeout(now);
    }
    fn fire_poll_timeout_internal(&self, eto: &mut Instant) {
        self.fire_poll_timeout(eto);
    }

    fn fire_read_eof_internal(&self) {
        self.fire_read_eof();
    }
    fn fire_exception_internal(&self, err: Box<dyn Error>) {
        self.fire_exception(err);
    }
    fn fire_close_internal(&self) {
        self.fire_close();
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_next_context(&mut self, next_context: Option<Rc<RefCell<dyn ContextInternal>>>) {
        self.next_context = next_context;
    }
    fn set_next_handler(&mut self, next_handler: Option<Rc<RefCell<dyn HandlerInternal>>>) {
        self.next_handler = next_handler;
    }
}
