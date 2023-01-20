use async_trait::async_trait;
use log::{trace, warn};
use std::any::Any;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Instant;

use crate::channel::handler_internal::{
    InboundHandlerContextInternal, InboundHandlerInternal, OutboundHandlerContextInternal,
    OutboundHandlerInternal,
};
use crate::error::Error;
use crate::runtime::sync::Mutex;

pub trait Handler: Send + Sync {
    #[allow(clippy::type_complexity)]
    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandlerContextInternal>>,
        Arc<Mutex<dyn InboundHandlerInternal>>,
        Arc<Mutex<dyn OutboundHandlerContextInternal>>,
        Arc<Mutex<dyn OutboundHandlerInternal>>,
    );
}

////////////////////////////////////////////////////////////////////////////////////////////////////
#[async_trait]
pub trait InboundHandler: Send + Sync {
    type Rin: Send + Sync + 'static;
    type Rout: Send + Sync + 'static;

    async fn transport_active(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_active().await;
    }
    async fn transport_inactive(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_transport_inactive().await;
    }

    async fn read(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        message: &mut Self::Rin,
    );
    async fn read_exception(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        error: Error,
    ) {
        ctx.fire_read_exception(error).await;
    }
    async fn read_eof(&mut self, ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>) {
        ctx.fire_read_eof().await;
    }

    async fn read_timeout(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        timeout: Instant,
    ) {
        ctx.fire_read_timeout(timeout).await;
    }
    async fn poll_timeout(
        &mut self,
        ctx: &mut InboundHandlerContext<Self::Rin, Self::Rout>,
        timeout: &mut Instant,
    ) {
        ctx.fire_poll_timeout(timeout).await;
    }
}

#[async_trait]
impl<Rin: Send + Sync + 'static, Rout: Send + Sync + 'static> InboundHandlerInternal
    for Box<dyn InboundHandler<Rin = Rin, Rout = Rout>>
{
    async fn transport_active_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.transport_active(ctx).await;
        } else {
            //TODO: panic!?
        }
    }
    async fn transport_inactive_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.transport_inactive(ctx).await;
        } else {
            //TODO: panic!?
        }
    }

    async fn read_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        message: &mut (dyn Any + Send + Sync),
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            if let Some(msg) = message.downcast_mut::<Rin>() {
                self.read(ctx, msg).await;
            } else {
                //TODO: panic!?
                ctx.fire_read_exception(Error::new(
                    ErrorKind::Other,
                    "message.downcast_mut error".to_string(),
                ))
                .await;
            }
        } else {
            //TODO: panic!?
        }
    }
    async fn read_exception_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        error: Error,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.read_exception(ctx, error).await;
        } else {
            //TODO: panic!?
        }
    }
    async fn read_eof_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<InboundHandlerContext<Rin, Rout>>()
        {
            self.read_eof(ctx).await;
        } else {
            //TODO: panic!?
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
            //TODO: panic!?
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
            //TODO: panic!?
        }
    }
}

#[async_trait]
pub trait OutboundHandler: Send + Sync {
    type Win: Send + Sync + 'static;
    type Wout: Send + Sync + 'static;

    async fn write(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        message: &mut Self::Win,
    );
    async fn write_exception(
        &mut self,
        ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>,
        error: Error,
    ) {
        ctx.fire_write_exception(error).await;
    }
    async fn close(&mut self, ctx: &mut OutboundHandlerContext<Self::Win, Self::Wout>) {
        ctx.fire_close().await;
    }
}

#[async_trait]
impl<Win: Send + Sync + 'static, Wout: Send + Sync + 'static> OutboundHandlerInternal
    for Box<dyn OutboundHandler<Win = Win, Wout = Wout>>
{
    async fn write_internal(
        &mut self,
        ctx: &mut dyn OutboundHandlerContextInternal,
        message: &mut (dyn Any + Send + Sync),
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<OutboundHandlerContext<Win, Wout>>()
        {
            if let Some(msg) = message.downcast_mut::<Win>() {
                self.write(ctx, msg).await;
            } else {
                //TODO: panic!?
                ctx.fire_write_exception(Error::new(
                    ErrorKind::Other,
                    "message.downcast_mut error".to_string(),
                ))
                .await;
            }
        } else {
            //TODO: panic!?
        }
    }
    async fn write_exception_internal(
        &mut self,
        ctx: &mut dyn OutboundHandlerContextInternal,
        error: Error,
    ) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<OutboundHandlerContext<Win, Wout>>()
        {
            self.write_exception(ctx, error).await;
        } else {
            //TODO: panic!?
        }
    }
    async fn close_internal(&mut self, ctx: &mut dyn OutboundHandlerContextInternal) {
        if let Some(ctx) = ctx
            .as_any()
            .downcast_mut::<OutboundHandlerContext<Win, Wout>>()
        {
            self.close(ctx).await;
        } else {
            //TODO: panic!?
        }
    }
}

#[derive(Default)]
pub struct InboundHandlerContext<Rin, Rout> {
    pub(crate) next_in_ctx: Option<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    pub(crate) next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,

    pub(crate) next_out: OutboundHandlerContext<Rout, Rin>,

    phantom_rin: PhantomData<Rin>,
    phantom_rout: PhantomData<Rout>,
}

impl<Rin: Send + Sync + 'static, Rout: Send + Sync + 'static> InboundHandlerContext<Rin, Rout> {
    pub async fn fire_transport_active(&mut self) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler.transport_active_internal(&mut *next_ctx).await;
        }
    }

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

    pub async fn fire_read(&mut self, message: &mut Rout) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler.read_internal(&mut *next_ctx, message).await;
        } else {
            warn!("read reached end of pipeline");
        }
    }

    pub async fn fire_read_exception(&mut self, error: Error) {
        if let (Some(next_in_handler), Some(next_in_ctx)) =
            (&self.next_in_handler, &self.next_in_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_in_handler.lock().await, next_in_ctx.lock().await);
            next_handler
                .read_exception_internal(&mut *next_ctx, error)
                .await;
        } else {
            warn!("read_exception reached end of pipeline");
        }
    }

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
impl<Rin: Send + Sync + 'static, Rout: Send + Sync + 'static> InboundHandlerContextInternal
    for InboundHandlerContext<Rin, Rout>
{
    async fn fire_transport_active_internal(&mut self) {
        self.fire_transport_active().await;
    }
    async fn fire_transport_inactive_internal(&mut self) {
        self.fire_transport_inactive().await;
    }
    async fn fire_read_internal(&mut self, message: &mut (dyn Any + Send + Sync)) {
        if let Some(msg) = message.downcast_mut::<Rout>() {
            self.fire_read(msg).await;
        } else {
            //TODO: panic!?
        }
    }
    async fn fire_read_exception_internal(&mut self, error: Error) {
        self.fire_read_exception(error).await;
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

#[derive(Default)]
pub struct OutboundHandlerContext<Win, Wout> {
    pub(crate) next_out_ctx: Option<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    pub(crate) next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,

    phantom_win: PhantomData<Win>,
    phantom_wout: PhantomData<Wout>,
}

impl<Win: Send + Sync + 'static, Wout: Send + Sync + 'static> OutboundHandlerContext<Win, Wout> {
    pub async fn fire_write(&mut self, message: &mut Wout) {
        if let (Some(next_out_handler), Some(next_out_ctx)) =
            (&self.next_out_handler, &self.next_out_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_out_handler.lock().await, next_out_ctx.lock().await);
            next_handler.write_internal(&mut *next_ctx, message).await;
        } else {
            warn!("write reached end of pipeline");
        }
    }

    pub async fn fire_write_exception(&mut self, error: Error) {
        if let (Some(next_out_handler), Some(next_out_ctx)) =
            (&self.next_out_handler, &self.next_out_ctx)
        {
            let (mut next_handler, mut next_ctx) =
                (next_out_handler.lock().await, next_out_ctx.lock().await);
            next_handler
                .write_exception_internal(&mut *next_ctx, error)
                .await;
        } else {
            warn!("write_exception reached end of pipeline");
        }
    }

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
impl<Win: Send + Sync + 'static, Wout: Send + Sync + 'static> OutboundHandlerContextInternal
    for OutboundHandlerContext<Win, Wout>
{
    async fn fire_write_internal(&mut self, message: &mut (dyn Any + Send + Sync)) {
        if let Some(msg) = message.downcast_mut::<Wout>() {
            self.fire_write(msg).await;
        } else {
            //TODO: panic!?
        }
    }
    async fn fire_write_exception_internal(&mut self, error: Error) {
        self.fire_write_exception(error).await;
    }
    async fn fire_close_internal(&mut self) {
        self.fire_close().await;
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
