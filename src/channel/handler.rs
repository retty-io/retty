use async_trait::async_trait;
use log::{trace, warn};
use std::any::Any;
use std::error::Error;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Instant;

use crate::channel::handler_internal::{
    InboundContextInternal, InboundHandlerInternal, OutboundContextInternal,
    OutboundHandlerInternal,
};
use crate::runtime::sync::Mutex;

/// Handles both inbound and outbound events and splits itself into InboundHandler and OutboundHandler
pub trait Handler: Send + Sync {
    /// Associated input message type for [InboundHandler::read]
    type Rin: Send + Sync + 'static;
    /// Associated output message type for [InboundHandler::read]
    type Rout: Send + Sync + 'static;
    /// Associated input message type for [OutboundHandler::write]
    type Win: Send + Sync + 'static;
    /// Associated output message type for [OutboundHandler::write]
    type Wout: Send + Sync + 'static;

    /// Returns handler name
    fn name(&self) -> &str;

    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    fn generate(
        self,
    ) -> (
        String,
        Arc<Mutex<dyn InboundContextInternal>>,
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundContextInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
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
            Arc::new(Mutex::new(inbound_context)),
            Arc::new(Mutex::new(inbound_handler)),
            Arc::new(Mutex::new(outbound_context)),
            Arc::new(Mutex::new(outbound_handler)),
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
#[async_trait]
pub trait InboundHandler: Send + Sync {
    /// Associated input message type for [InboundHandler::read]
    type Rin: Send + Sync + 'static;
    /// Associated output message type for [InboundHandler::read]
    type Rout: Send + Sync + 'static;

    /// Transport is active now, which means it is connected.
    async fn transport_active(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_active().await;
    }
    /// Transport is inactive now, which means it is disconnected.
    async fn transport_inactive(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_inactive().await;
    }

    /// Reads a message.
    async fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin);
    /// Reads an Error exception in one of its inbound operations.
    async fn read_exception(
        &mut self,
        ctx: &InboundContext<Self::Rin, Self::Rout>,
        err: Box<dyn Error + Send + Sync>,
    ) {
        ctx.fire_read_exception(err).await;
    }
    /// Reads an EOF event.
    async fn read_eof(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>) {
        ctx.fire_read_eof().await;
    }

    /// Reads a timeout event.
    async fn read_timeout(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, now: Instant) {
        ctx.fire_read_timeout(now).await;
    }
    /// Polls earliest timeout (eto) in its inbound operations.
    async fn poll_timeout(
        &mut self,
        ctx: &InboundContext<Self::Rin, Self::Rout>,
        eto: &mut Instant,
    ) {
        ctx.fire_poll_timeout(eto).await;
    }
}

#[async_trait]
impl<Rin: Send + Sync + 'static, Rout: Send + Sync + 'static> InboundHandlerInternal
    for Box<dyn InboundHandler<Rin = Rin, Rout = Rout>>
{
    async fn transport_active_internal(&mut self, ctx: &dyn InboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.transport_active(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn transport_inactive_internal(&mut self, ctx: &dyn InboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.transport_inactive(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }

    async fn read_internal(
        &mut self,
        ctx: &dyn InboundContextInternal,
        msg: Box<dyn Any + Send + Sync>,
    ) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            if let Ok(msg) = msg.downcast::<Rin>() {
                self.read(ctx, *msg).await;
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
    async fn read_exception_internal(
        &mut self,
        ctx: &dyn InboundContextInternal,
        err: Box<dyn Error + Send + Sync>,
    ) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.read_exception(ctx, err).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn read_eof_internal(&mut self, ctx: &dyn InboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.read_eof(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }

    async fn read_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, now: Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.read_timeout(ctx, now).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn poll_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, eto: &mut Instant) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<InboundContext<Rin, Rout>>() {
            self.poll_timeout(ctx, eto).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<InboundContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
}

/// Handles an outbound I/O event or intercepts an I/O operation, and forwards it to its next outbound handler in its Pipeline.
#[async_trait]
pub trait OutboundHandler: Send + Sync {
    /// Associated input message type for [OutboundHandler::write]
    type Win: Send + Sync + 'static;
    /// Associated output message type for [OutboundHandler::write]
    type Wout: Send + Sync + 'static;

    /// Writes a message.
    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win);
    /// Writes an Error exception from one of its outbound operations.
    async fn write_exception(
        &mut self,
        ctx: &OutboundContext<Self::Win, Self::Wout>,
        err: Box<dyn Error + Send + Sync>,
    ) {
        ctx.fire_write_exception(err).await;
    }
    /// Writes a close event.
    async fn close(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>) {
        ctx.fire_close().await;
    }
}

#[async_trait]
impl<Win: Send + Sync + 'static, Wout: Send + Sync + 'static> OutboundHandlerInternal
    for Box<dyn OutboundHandler<Win = Win, Wout = Wout>>
{
    async fn write_internal(
        &mut self,
        ctx: &dyn OutboundContextInternal,
        msg: Box<dyn Any + Send + Sync>,
    ) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<OutboundContext<Win, Wout>>() {
            if let Ok(msg) = msg.downcast::<Win>() {
                self.write(ctx, *msg).await;
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
    async fn write_exception_internal(
        &mut self,
        ctx: &dyn OutboundContextInternal,
        err: Box<dyn Error + Send + Sync>,
    ) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<OutboundContext<Win, Wout>>() {
            self.write_exception(ctx, err).await;
        } else {
            panic!(
                "ctx can't downcast_ref::<OutboundContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn close_internal(&mut self, ctx: &dyn OutboundContextInternal) {
        if let Some(ctx) = ctx.as_any().downcast_ref::<OutboundContext<Win, Wout>>() {
            self.close(ctx).await;
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

    next_in_context: Option<Arc<Mutex<dyn InboundContextInternal>>>,
    next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,

    next_out: OutboundContext<Rout, Rin>,

    phantom_rin: PhantomData<Rin>,
    phantom_rout: PhantomData<Rout>,
}

impl<Rin: Send + Sync + 'static, Rout: Send + Sync + 'static> InboundContext<Rin, Rout> {
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
    pub async fn fire_transport_active(&self) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.transport_active_internal(&*next_ctx).await;
        }
    }

    /// Transport is inactive now, which means it is disconnected.
    pub async fn fire_transport_inactive(&self) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.transport_inactive_internal(&*next_ctx).await;
        }
    }

    /// Reads a message.
    pub async fn fire_read(&self, msg: Rout) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.read_internal(&*next_ctx, Box::new(msg)).await;
        } else {
            warn!("read reached end of pipeline");
        }
    }

    /// Reads an Error exception in one of its inbound operations.
    pub async fn fire_read_exception(&self, err: Box<dyn Error + Send + Sync>) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.read_exception_internal(&*next_ctx, err).await;
        } else {
            warn!("read_exception reached end of pipeline");
        }
    }

    /// Reads an EOF event.
    pub async fn fire_read_eof(&self) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.read_eof_internal(&*next_ctx).await;
        } else {
            warn!("read_eof reached end of pipeline");
        }
    }

    /// Reads a timeout event.
    pub async fn fire_read_timeout(&self, now: Instant) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.read_timeout_internal(&*next_ctx, now).await;
        } else {
            warn!("read reached end of pipeline");
        }
    }

    /// Polls earliest timeout (eto) in its inbound operations.
    pub async fn fire_poll_timeout(&self, eto: &mut Instant) {
        if let (Some(next_in_handler), Some(next_in_context)) =
            (&self.next_in_handler, &self.next_in_context)
        {
            let (mut next_handler, next_ctx) =
                (next_in_handler.lock().await, next_in_context.lock().await);
            next_handler.poll_timeout_internal(&*next_ctx, eto).await;
        } else {
            trace!("poll_timeout reached end of pipeline");
        }
    }
}

#[async_trait]
impl<Rin: Send + Sync + 'static, Rout: Send + Sync + 'static> InboundContextInternal
    for InboundContext<Rin, Rout>
{
    async fn fire_transport_active_internal(&self) {
        self.fire_transport_active().await;
    }
    async fn fire_transport_inactive_internal(&self) {
        self.fire_transport_inactive().await;
    }
    async fn fire_read_internal(&self, msg: Box<dyn Any + Send + Sync>) {
        if let Ok(msg) = msg.downcast::<Rout>() {
            self.fire_read(*msg).await;
        } else {
            panic!("msg can't downcast::<Rout> in {} handler", self.name());
        }
    }
    async fn fire_read_exception_internal(&self, err: Box<dyn Error + Send + Sync>) {
        self.fire_read_exception(err).await;
    }
    async fn fire_read_eof_internal(&self) {
        self.fire_read_eof().await;
    }
    async fn fire_read_timeout_internal(&self, now: Instant) {
        self.fire_read_timeout(now).await;
    }
    async fn fire_poll_timeout_internal(&self, eto: &mut Instant) {
        self.fire_poll_timeout(eto).await;
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
    fn set_next_in_context(
        &mut self,
        next_in_context: Option<Arc<Mutex<dyn InboundContextInternal>>>,
    ) {
        self.next_in_context = next_in_context;
    }
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,
    ) {
        self.next_in_handler = next_in_handler;
    }
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Arc<Mutex<dyn OutboundContextInternal>>>,
    ) {
        self.next_out_context = next_out_context;
    }
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
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

    next_out_context: Option<Arc<Mutex<dyn OutboundContextInternal>>>,
    next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,

    phantom_win: PhantomData<Win>,
    phantom_wout: PhantomData<Wout>,
}

impl<Win: Send + Sync + 'static, Wout: Send + Sync + 'static> OutboundContext<Win, Wout> {
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
    pub async fn fire_write(&self, msg: Wout) {
        if let (Some(next_out_handler), Some(next_out_context)) =
            (&self.next_out_handler, &self.next_out_context)
        {
            let (mut next_handler, next_ctx) =
                (next_out_handler.lock().await, next_out_context.lock().await);
            next_handler.write_internal(&*next_ctx, Box::new(msg)).await;
        } else {
            warn!("write reached end of pipeline");
        }
    }

    /// Writes an Error exception from one of its outbound operations.
    pub async fn fire_write_exception(&self, err: Box<dyn Error + Send + Sync>) {
        if let (Some(next_out_handler), Some(next_out_context)) =
            (&self.next_out_handler, &self.next_out_context)
        {
            let (mut next_handler, next_ctx) =
                (next_out_handler.lock().await, next_out_context.lock().await);
            next_handler.write_exception_internal(&*next_ctx, err).await;
        } else {
            warn!("write_exception reached end of pipeline");
        }
    }

    /// Writes a close event.
    pub async fn fire_close(&self) {
        if let (Some(next_out_handler), Some(next_out_context)) =
            (&self.next_out_handler, &self.next_out_context)
        {
            let (mut next_handler, next_ctx) =
                (next_out_handler.lock().await, next_out_context.lock().await);
            next_handler.close_internal(&*next_ctx).await;
        } else {
            warn!("close reached end of pipeline");
        }
    }
}

#[async_trait]
impl<Win: Send + Sync + 'static, Wout: Send + Sync + 'static> OutboundContextInternal
    for OutboundContext<Win, Wout>
{
    async fn fire_write_internal(&self, msg: Box<dyn Any + Send + Sync>) {
        if let Ok(msg) = msg.downcast::<Wout>() {
            self.fire_write(*msg).await;
        } else {
            panic!("msg can't downcast::<Wout> in {} handler", self.name());
        }
    }
    async fn fire_write_exception_internal(&self, err: Box<dyn Error + Send + Sync>) {
        self.fire_write_exception(err).await;
    }
    async fn fire_close_internal(&self) {
        self.fire_close().await;
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Arc<Mutex<dyn OutboundContextInternal>>>,
    ) {
        self.next_out_context = next_out_context;
    }
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
    ) {
        self.next_out_handler = next_out_handler;
    }
}
