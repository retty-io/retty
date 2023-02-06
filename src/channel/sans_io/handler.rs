use log::{trace, warn};
use std::any::Any;
use std::cell::RefCell;
use std::error::Error;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::time::Instant;

use crate::channel::sans_io::handler_internal::{
    InboundContextInternal, InboundHandlerInternal, OutboundContextInternal,
    OutboundHandlerInternal,
};

/// Handles both inbound and outbound events and splits itself into InboundHandler and OutboundHandler
pub trait Handler {
    /// Associated input message type for [InboundHandler::read]
    type Rin: 'static;
    /// Associated output message type for [InboundHandler::read]
    type Rout: 'static;
    /// Associated input message type for [OutboundHandler::write]
    type Win: 'static;
    /// Associated output message type for [OutboundHandler::write]
    type Wout: 'static;

    /// Returns handler name
    fn name(&self) -> &str;

    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    fn generate(
        self,
    ) -> (
        String,
        Rc<RefCell<dyn InboundContextInternal>>,
        Rc<RefCell<dyn InboundHandlerInternal>>,
        Rc<RefCell<dyn OutboundContextInternal>>,
        Rc<RefCell<dyn OutboundHandlerInternal>>,
    )
    where
        Self: Sized,
    {
        let handler_name = self.name().to_owned();
        let inbound_context: InboundContext<Self::Rin, Self::Rout> =
            InboundContext::new(self.name());
        let outbound_context: OutboundContext<Self::Win, Self::Wout> =
            OutboundContext::new(self.name());

        let (inbound_handler, outbound_handler) = self.split();

        (
            handler_name,
            Rc::new(RefCell::new(inbound_context)),
            Rc::new(RefCell::new(inbound_handler)),
            Rc::new(RefCell::new(outbound_context)),
            Rc::new(RefCell::new(outbound_handler)),
        )
    }

    /// Splits itself into InboundHandler and OutboundHandler
    #[allow(clippy::type_complexity)]
    fn split(
        self,
    ) -> (
        Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
        Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
    );
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Handles an inbound I/O event or intercepts an I/O operation, and forwards it to its next inbound handler in its Pipeline.
pub trait InboundHandler {
    /// Associated input message type for [InboundHandler::read]
    type Rin: 'static;
    /// Associated output message type for [InboundHandler::read]
    type Rout: 'static;

    /// Transport is active now, which means it is connected.
    fn transport_active(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_active();
    }
    /// Transport is inactive now, which means it is disconnected.
    fn transport_inactive(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_inactive();
    }

    /// Reads a message.
    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin);
    /// Reads an Error exception in one of its inbound operations.
    fn read_exception(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, err: Box<dyn Error>) {
        ctx.fire_read_exception(err);
    }
    /// Reads an EOF event.
    fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_read_eof();
    }

    /// Reads a timeout event.
    fn read_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        ctx.fire_read_timeout(now);
    }
    /// Polls earliest timeout (eto) in its inbound operations.
    fn poll_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, eto: &mut Instant) {
        ctx.fire_poll_timeout(eto);
    }
}

impl<Rin: 'static, Rout: 'static> InboundHandlerInternal
    for Box<dyn InboundHandler<Rin = Rin, Rout = Rout>>
{
    fn transport_active_internal(&mut self, ctx: &dyn InboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.transport_active(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn transport_inactive_internal(&mut self, ctx: &dyn InboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.transport_inactive(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn read_internal(&mut self, ctx: &dyn InboundContextInternal, msg: Box<dyn Any>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            if let Ok(msg) = msg.downcast::<Rin>() {
                self.read(ctx, *msg);
            } else {
                panic!("msg can't downcast::<Rin> in {} handler", ctx.name());
            }
        } else {
            panic!(
                "ctx can't downcast::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn read_exception_internal(&mut self, ctx: &dyn InboundContextInternal, err: Box<dyn Error>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.read_exception(ctx, err);
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn read_eof_internal(&mut self, ctx: &dyn InboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.read_eof(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }

    fn read_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, now: Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.read_timeout(ctx, now);
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn poll_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, eto: &mut Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.poll_timeout(ctx, eto);
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
}

/// Handles an outbound I/O event or intercepts an I/O operation, and forwards it to its next outbound handler in its Pipeline.
pub trait OutboundHandler {
    /// Associated input message type for [OutboundHandler::write]
    type Win: 'static;
    /// Associated output message type for [OutboundHandler::write]
    type Wout: 'static;

    /// Writes a message.
    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win);
    /// Writes an Error exception from one of its outbound operations.
    fn write_exception(
        &mut self,
        ctx: &OutboundContext<Self::Win, Self::Wout>,
        err: Box<dyn Error>,
    ) {
        ctx.fire_write_exception(err);
    }
    /// Writes a close event.
    fn close(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>) {
        ctx.fire_close();
    }
}

impl<Win: 'static, Wout: 'static> OutboundHandlerInternal
    for Box<dyn OutboundHandler<Win = Win, Wout = Wout>>
{
    fn write_internal(&mut self, ctx: &dyn OutboundContextInternal, msg: Box<dyn Any>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<OutboundContext<Win, Wout>>() {
            if let Ok(msg) = msg.downcast::<Win>() {
                self.write(ctx, *msg);
            } else {
                panic!("msg can't downcast::<Win> in {} handler", ctx.name());
            }
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn write_exception_internal(&mut self, ctx: &dyn OutboundContextInternal, err: Box<dyn Error>) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<OutboundContext<Win, Wout>>() {
            self.write_exception(ctx, err);
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    fn close_internal(&mut self, ctx: &dyn OutboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<OutboundContext<Win, Wout>>() {
            self.close(ctx);
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
}

/// Enables a [InboundHandler] to interact with its Pipeline and other handlers.
pub struct InboundContext<Rin, Rout> {
    name: String,

    next_in_context: Option<Rc<RefCell<dyn InboundContextInternal>>>,
    next_in_handler: Option<Rc<RefCell<dyn InboundHandlerInternal>>>,

    next_out: OutboundContext<Rout, Rin>,

    phantom_rin: PhantomData<Rin>,
    phantom_rout: PhantomData<Rout>,
}

impl<Rin: 'static, Rout: 'static> InboundContext<Rin, Rout> {
    /// Creates a new InboundContext
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),

            next_in_context: None,
            next_in_handler: None,

            next_out: OutboundContext::new(name),

            phantom_rin: PhantomData,
            phantom_rout: PhantomData,
        }
    }

    /// Transport is active now, which means it is connected.
    pub fn fire_transport_active(&self) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.transport_active_internal(&*next_ctx);
        }
    }

    /// Transport is inactive now, which means it is disconnected.
    pub fn fire_transport_inactive(&self) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.transport_inactive_internal(&*next_ctx);
        }
    }

    /// Reads a message.
    pub fn fire_read(&self, msg: Rout) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.read_internal(&*next_ctx, Box::new(msg));
        } else {
            warn!("read reached end of pipeline");
        }
    }

    /// Reads an Error exception in one of its inbound operations.
    pub fn fire_read_exception(&self, err: Box<dyn Error>) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.read_exception_internal(&*next_ctx, err);
        } else {
            warn!("read_exception reached end of pipeline");
        }
    }

    /// Reads an EOF event.
    pub fn fire_read_eof(&self) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.read_eof_internal(&*next_ctx);
        } else {
            warn!("read_eof reached end of pipeline");
        }
    }

    /// Reads a timeout event.
    pub fn fire_read_timeout(&self, now: Instant) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.read_timeout_internal(&*next_ctx, now);
        } else {
            warn!("read reached end of pipeline");
        }
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    pub fn fire_poll_timeout(&self, eto: &mut Instant) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.borrow_mut(), next_in_context.borrow());
            next_handler.poll_timeout_internal(&*next_ctx, eto);
        } else {
            trace!("poll_timeout reached end of pipeline");
        }
    }
}

impl<Rin: 'static, Rout: 'static> InboundContextInternal for InboundContext<Rin, Rout> {
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
    fn fire_read_exception_internal(&self, err: Box<dyn Error>) {
        self.fire_read_exception(err);
    }
    fn fire_read_eof_internal(&self) {
        self.fire_read_eof();
    }
    fn fire_read_timeout_internal(&self, now: Instant) {
        self.fire_read_timeout(now);
    }
    fn fire_poll_timeout_internal(&self, eto: &mut Instant) {
        self.fire_poll_timeout(eto);
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_next_in_context(
        &mut self,
        next_in_context: Option<Rc<RefCell<dyn InboundContextInternal>>>,
    ) {
        self.next_in_context = next_in_context;
    }
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Rc<RefCell<dyn InboundHandlerInternal>>>,
    ) {
        self.next_in_handler = next_in_handler;
    }
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    ) {
        self.next_out_context = next_out_context;
    }
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    ) {
        self.next_out_handler = next_out_handler;
    }
}

impl<Rin, Rout> Deref for InboundContext<Rin, Rout> {
    type Target = OutboundContext<Rout, Rin>;
    fn deref(&self) -> &Self::Target {
        &self.next_out
    }
}

impl<Rin, Rout> DerefMut for InboundContext<Rin, Rout> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.next_out
    }
}

/// Enables a [OutboundHandler] to interact with its Pipeline and other handlers.
pub struct OutboundContext<Win, Wout> {
    name: String,

    next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,

    phantom_win: PhantomData<Win>,
    phantom_wout: PhantomData<Wout>,
}

impl<Win: 'static, Wout: 'static> OutboundContext<Win, Wout> {
    /// Creates a new OutboundContext
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),

            next_out_context: None,
            next_out_handler: None,

            phantom_win: PhantomData,
            phantom_wout: PhantomData,
        }
    }

    /// Writes a message.
    pub fn fire_write(&self, msg: Wout) {
        if let (Some(next_out_handler), Some(next_out_context)) =
            (&self.next_out_handler, &self.next_out_context)
        {
            let (mut next_handler, next_ctx) =
                (next_out_handler.borrow_mut(), next_out_context.borrow());
            next_handler.write_internal(&*next_ctx, Box::new(msg));
        } else {
            warn!("write reached end of pipeline");
        }
    }

    /// Writes an Error exception from one of its outbound operations.
    pub fn fire_write_exception(&self, err: Box<dyn Error>) {
        if let (Some(next_out_handler), Some(next_out_context)) =
            (&self.next_out_handler, &self.next_out_context)
        {
            let (mut next_handler, next_ctx) =
                (next_out_handler.borrow_mut(), next_out_context.borrow());
            next_handler.write_exception_internal(&*next_ctx, err);
        } else {
            warn!("write_exception reached end of pipeline");
        }
    }

    /// Writes a close event.
    pub fn fire_close(&self) {
        if let (Some(next_out_handler), Some(next_out_context)) =
            (&self.next_out_handler, &self.next_out_context)
        {
            let (mut next_handler, next_ctx) =
                (next_out_handler.borrow_mut(), next_out_context.borrow());
            next_handler.close_internal(&*next_ctx);
        } else {
            warn!("close reached end of pipeline");
        }
    }
}

impl<Win: 'static, Wout: 'static> OutboundContextInternal for OutboundContext<Win, Wout> {
    fn fire_write_internal(&self, msg: Box<dyn Any>) {
        if let Ok(msg) = msg.downcast::<Wout>() {
            self.fire_write(*msg);
        } else {
            panic!("msg can't downcast::<Wout> in {} handler", self.name());
        }
    }
    fn fire_write_exception_internal(&self, err: Box<dyn Error>) {
        self.fire_write_exception(err);
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
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Rc<RefCell<dyn OutboundContextInternal>>>,
    ) {
        self.next_out_context = next_out_context;
    }
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Rc<RefCell<dyn OutboundHandlerInternal>>>,
    ) {
        self.next_out_handler = next_out_handler;
    }
}
