use async_trait::async_trait;
use log::{trace, warn};
use std::any::Any;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Instant;

use crate::channel::handler_internal::{
    InboundHandlerContextInternal, InboundHandlerInternal, MessageInternal,
    OutboundHandlerContextInternal, OutboundHandlerInternal,
};
use crate::error::Error;
use crate::runtime::sync::Mutex;

/// Handles both inbound and outbound events and splits itself into InboundHandler and OutboundHandler
pub trait Handler: Send + Sync {
    /// Associated input message type for [InboundHandler::read]
    type Rin: Default + Send + Sync + 'static;
    /// Associated output message type for [InboundHandler::read]
    type Rout: Default + Send + Sync + 'static;
    /// Associated input message type for [OutboundHandler::write]
    type Win: Default + Send + Sync + 'static;
    /// Associated output message type for [OutboundHandler::write]
    type Wout: Default + Send + Sync + 'static;

    /// Returns handler name
    fn name(&self) -> &str;

    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    fn generate(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerContextInternal>>,
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerContextInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    )
    where
        Self: Sized,
    {
        let inbound_context: InboundHandlerContext<Self::Rin, Self::Rout> =
            InboundHandlerContext::new(self.name());
        let outbound_context: OutboundHandlerContext<Self::Win, Self::Wout> =
            OutboundHandlerContext::new(self.name());

        let (inbound_handler, outbound_handler) = self.split();

        (
            Arc::new(Mutex::new(inbound_context)),
            inbound_handler,
            Arc::new(Mutex::new(outbound_context)),
            outbound_handler,
        )
    }

    /// Splits itself into InboundHandler and OutboundHandler
    #[allow(clippy::type_complexity)]
    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    );
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Handles an inbound I/O event or intercepts an I/O operation, and forwards it to its next inbound handler in its Pipeline.
#[async_trait]
pub trait InboundHandler: Send + Sync {
    /// Associated input message type for [InboundHandler::read]
    type Rin: Default + Send + Sync + 'static;
    /// Associated output message type for [InboundHandler::read]
    type Rout: Default + Send + Sync + 'static;

    /// Transport is active now, which means it is connected.
    async fn transport_active(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_active().await;
    }
    /// Transport is inactive now, which means it is disconnected.
    async fn transport_inactive(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_inactive().await;
    }

    /// Reads a message.
    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        msg: &mut Self::Rin,
    );
    /// Reads an [Error] exception in one of its inbound operations.
    async fn read_exception(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        err: Error,
    ) {
        ctx.fire_read_exception(err).await;
    }
    /// Reads an EOF event.
    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_read_eof().await;
    }

    /// Reads a timeout event.
    async fn read_timeout(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        timeout: Instant,
    ) {
        ctx.fire_read_timeout(timeout).await;
    }
    /// Polls timout event in its inbound operations.
    /// If any inbound handler has timeout event to trigger in future,
    /// it should compare its own timeout event with the provided timout event and
    /// update the provided timeout event with the minimal of these two timeout events.
    async fn poll_timeout(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        timeout: &mut Instant,
    ) {
        ctx.fire_poll_timeout(timeout).await;
    }
}

#[async_trait]
impl<Rin: Default + Send + Sync + 'static, Rout: Default + Send + Sync + 'static>
    InboundHandlerInternal for Box<dyn InboundHandler<Rin = Rin, Rout = Rout>>
{
    async fn transport_active_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.transport_active(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn transport_inactive_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.transport_inactive(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }

    async fn read_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        msg: &mut MessageInternal,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            if let Some(msg) = msg.downcast_mut::<Rin>() {
                self.read(ctx, msg).await;
            } else {
                panic!("msg can't downcast_mut::<Rin> in {} handler", ctx.name());
            }
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn read_exception_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        err: Error,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.read_exception(ctx, err).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn read_eof_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.read_eof(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }

    async fn read_timeout_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        timeout: Instant,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.read_timeout(ctx, timeout).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn poll_timeout_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        timeout: &mut Instant,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.poll_timeout(ctx, timeout).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<InboundHandlerContext<Rin, Rout>> in {} handler",
                ctx.name()
            );
        }
    }
}

/// Handles an outbound I/O event or intercepts an I/O operation, and forwards it to its next outbound handler in its Pipeline.
#[async_trait]
pub trait OutboundHandler: Send + Sync {
    /// Associated input message type for [OutboundHandler::write]
    type Win: Default + Send + Sync + 'static;
    /// Associated output message type for [OutboundHandler::write]
    type Wout: Default + Send + Sync + 'static;

    /// Writes a message.
    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        msg: &mut Self::Win,
    );
    /// Writes an [Error] exception from one of its outbound operations.
    async fn write_exception(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        err: Error,
    ) {
        ctx.fire_write_exception(err).await;
    }
    /// Writes a close event.
    async fn close(&mut self, ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>) {
        ctx.fire_close().await;
    }
}

#[async_trait]
impl<Win: Default + Send + Sync + 'static, Wout: Default + Send + Sync + 'static>
    OutboundHandlerInternal for Box<dyn OutboundHandler<Win = Win, Wout = Wout>>
{
    async fn write_internal(
        &mut self,
        ctx: &mut dyn OutboundHandlerContextInternal,
        msg: &mut MessageInternal,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<OutboundHandlerContext<Win, Wout>>()
        {
            if let Some(msg) = msg.downcast_mut::<Win>() {
                self.write(ctx, msg).await;
            } else {
                panic!("msg can't downcast_mut::<Win> in {} handler", ctx.name());
            }
        } else {
            panic!(
                "ctx can't downcast_mut::<OutboundHandlerContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn write_exception_internal(
        &mut self,
        ctx: &mut dyn OutboundHandlerContextInternal,
        err: Error,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<OutboundHandlerContext<Win, Wout>>()
        {
            self.write_exception(ctx, err).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<OutboundHandlerContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
    async fn close_internal(&mut self, ctx: &mut dyn OutboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<OutboundHandlerContext<Win, Wout>>()
        {
            self.close(ctx).await;
        } else {
            panic!(
                "ctx can't downcast_mut::<OutboundHandlerContext<Win, Wout>> in {} handler",
                ctx.name()
            );
        }
    }
}

/// Enables a [InboundHandler] to interact with its Pipeline and other handlers.
#[derive(Default)]
pub struct InboundHandlerContext<Rin, Rout> {
    name: String,

    next_in_ctx: Option<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,

    next_out: OutboundHandlerContext<Rout, Rin>,

    phantom_rin: PhantomData<Rin>,
    phantom_rout: PhantomData<Rout>,
}

impl<Rin: Default + Send + Sync + 'static, Rout: Default + Send + Sync + 'static>
    InboundHandlerContext<Rin, Rout>
{
    /// Creates a new InboundHandlerContext
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Transport is active now, which means it is connected.
    pub async fn fire_transport_active(&mut self) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler.transport_active_internal(&mut *next_ctx).await;
        }
    }

    /// Transport is inactive now, which means it is disconnected.
    pub async fn fire_transport_inactive(&mut self) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler
                .transport_inactive_internal(&mut *next_ctx)
                .await;
        }
    }

    /// Reads a message.
    pub async fn fire_read(&mut self, msg: &mut Rout) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler.read_internal(&mut *next_ctx, msg).await;
        } else {
            warn!("read reached end of pipeline");
        }
    }

    /// Reads an [Error] exception in one of its inbound operations.
    pub async fn fire_read_exception(&mut self, err: Error) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler
                .read_exception_internal(&mut *next_ctx, err)
                .await;
        } else {
            warn!("read_exception reached end of pipeline");
        }
    }

    /// Reads an EOF event.
    pub async fn fire_read_eof(&mut self) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler.read_eof_internal(&mut *next_ctx).await;
        } else {
            warn!("read_eof reached end of pipeline");
        }
    }

    /// Reads a timeout event.
    pub async fn fire_read_timeout(&mut self, timeout: Instant) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler
                .read_timeout_internal(&mut *next_ctx, timeout)
                .await;
        } else {
            warn!("read reached end of pipeline");
        }
    }

    /// Polls timout event in its inbound operations.
    /// If any inbound handler has timeout event to trigger in future,
    /// it should compare its own timeout event with the provided timout event and
    /// update the provided timeout event with the minimal of these two timeout events.
    pub async fn fire_poll_timeout(&mut self, timeout: &mut Instant) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler
                .poll_timeout_internal(&mut *next_ctx, timeout)
                .await;
        } else {
            trace!("poll_timeout reached end of pipeline");
        }
    }
}

#[async_trait]
impl<Rin: Default + Send + Sync + 'static, Rout: Default + Send + Sync + 'static>
    InboundHandlerContextInternal for InboundHandlerContext<Rin, Rout>
{
    async fn fire_transport_active_internal(&mut self) {
        self.fire_transport_active().await;
    }
    async fn fire_transport_inactive_internal(&mut self) {
        self.fire_transport_inactive().await;
    }
    async fn fire_read_internal(&mut self, msg: &mut MessageInternal) {
        if let Some(msg) = msg.downcast_mut::<Rout>() {
            self.fire_read(msg).await;
        } else {
            panic!("msg can't downcast_mut::<Rout> in {} handler", self.name());
        }
    }
    async fn fire_read_exception_internal(&mut self, err: Error) {
        self.fire_read_exception(err).await;
    }
    async fn fire_read_eof_internal(&mut self) {
        self.fire_read_eof().await;
    }
    async fn fire_read_timeout_internal(&mut self, timeout: Instant) {
        self.fire_read_timeout(timeout).await;
    }
    async fn fire_poll_timeout_internal(&mut self, timeout: &mut Instant) {
        self.fire_poll_timeout(timeout).await;
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
    fn set_next_in_ctx(
        &mut self,
        next_in_ctx: Option<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    ) {
        self.next_in_ctx = next_in_ctx;
    }
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,
    ) {
        self.next_in_handler = next_in_handler;
    }
    fn set_next_out_ctx(
        &mut self,
        next_out_ctx: Option<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    ) {
        self.next_out_ctx = next_out_ctx;
    }
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
    ) {
        self.next_out_handler = next_out_handler;
    }
}

impl<Rin, Rout> Deref for InboundHandlerContext<Rin, Rout> {
    type Target = OutboundHandlerContext<Rout, Rin>;
    fn deref(&self) -> &Self::Target {
        &self.next_out
    }
}

impl<Rin, Rout> DerefMut for InboundHandlerContext<Rin, Rout> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.next_out
    }
}

/// Enables a [OutboundHandler] to interact with its Pipeline and other handlers.
#[derive(Default)]
pub struct OutboundHandlerContext<Win, Wout> {
    name: String,

    next_out_ctx: Option<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,

    phantom_win: PhantomData<Win>,
    phantom_wout: PhantomData<Wout>,
}

impl<Win: Default + Send + Sync + 'static, Wout: Default + Send + Sync + 'static>
    OutboundHandlerContext<Win, Wout>
{
    /// Creates a new OutboundHandlerContext
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Writes a message.
    pub async fn fire_write(&mut self, msg: &mut Wout) {
        if let (Some(next_out_handler), Some(next_out_ctx)) =
            (&self.next_out_handler, &self.next_out_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_out_handler.lock().await, next_out_ctx.lock().await);
            next_handler.write_internal(&mut *next_ctx, msg).await;
        } else {
            warn!("write reached end of pipeline");
        }
    }

    /// Writes an [Error] exception from one of its outbound operations.
    pub async fn fire_write_exception(&mut self, err: Error) {
        if let (Some(next_out_handler), Some(next_out_ctx)) =
            (&self.next_out_handler, &self.next_out_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_out_handler.lock().await, next_out_ctx.lock().await);
            next_handler
                .write_exception_internal(&mut *next_ctx, err)
                .await;
        } else {
            warn!("write_exception reached end of pipeline");
        }
    }

    /// Writes a close event.
    pub async fn fire_close(&mut self) {
        if let (Some(next_out_handler), Some(next_out_ctx)) =
            (&self.next_out_handler, &self.next_out_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_out_handler.lock().await, next_out_ctx.lock().await);
            next_handler.close_internal(&mut *next_ctx).await;
        } else {
            warn!("close reached end of pipeline");
        }
    }
}

#[async_trait]
impl<Win: Default + Send + Sync + 'static, Wout: Default + Send + Sync + 'static>
    OutboundHandlerContextInternal for OutboundHandlerContext<Win, Wout>
{
    async fn fire_write_internal(&mut self, msg: &mut MessageInternal) {
        if let Some(msg) = msg.downcast_mut::<Wout>() {
            self.fire_write(msg).await;
        } else {
            panic!("msg can't downcast_mut::<Wout> in {} handler", self.name());
        }
    }
    async fn fire_write_exception_internal(&mut self, err: Error) {
        self.fire_write_exception(err).await;
    }
    async fn fire_close_internal(&mut self) {
        self.fire_close().await;
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }
    fn as_any(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
    fn set_next_out_ctx(
        &mut self,
        next_out_ctx: Option<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    ) {
        self.next_out_ctx = next_out_ctx;
    }
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
    ) {
        self.next_out_handler = next_out_handler;
    }
}
