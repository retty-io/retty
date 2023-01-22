use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use std::time::Instant;

use crate::error::Error;
use crate::runtime::sync::Mutex;

pub(crate) type MessageInternal = dyn Any + Send + Sync;

#[doc(hidden)]
#[async_trait]
pub trait InboundHandlerInternal: Send + Sync {
    async fn transport_active_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal);
    async fn transport_inactive_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal);

    async fn read_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        msg: &mut MessageInternal,
    );
    async fn read_exception_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        err: Error,
    );
    async fn read_eof_internal(&mut self, ctx: &mut dyn InboundHandlerContextInternal);

    async fn read_timeout_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        timeout: Instant,
    );
    async fn poll_timeout_internal(
        &mut self,
        ctx: &mut dyn InboundHandlerContextInternal,
        timeout: &mut Instant,
    );
}

#[doc(hidden)]
#[async_trait]
pub trait InboundHandlerContextInternal: Send + Sync {
    async fn fire_transport_active_internal(&mut self);
    async fn fire_transport_inactive_internal(&mut self);
    async fn fire_read_internal(&mut self, msg: &mut MessageInternal);
    async fn fire_read_exception_internal(&mut self, err: Error);
    async fn fire_read_eof_internal(&mut self);
    async fn fire_read_timeout_internal(&mut self, timeout: Instant);
    async fn fire_poll_timeout_internal(&mut self, timeout: &mut Instant);

    fn name(&self) -> &str;
    fn as_any(&mut self) -> &mut (dyn Any + Send + Sync);
    fn set_next_in_ctx(
        &mut self,
        next_in_ctx: Option<Arc<Mutex<dyn InboundHandlerContextInternal>>>,
    );
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,
    );
    fn set_next_out_ctx(
        &mut self,
        next_out_ctx: Option<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    );
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
    );
}

#[doc(hidden)]
#[async_trait]
pub trait OutboundHandlerInternal: Send + Sync {
    async fn write_internal(
        &mut self,
        ctx: &mut dyn OutboundHandlerContextInternal,
        msg: &mut MessageInternal,
    );
    async fn write_exception_internal(
        &mut self,
        ctx: &mut dyn OutboundHandlerContextInternal,
        err: Error,
    );
    async fn close_internal(&mut self, ctx: &mut dyn OutboundHandlerContextInternal);
}

#[doc(hidden)]
#[async_trait]
pub trait OutboundHandlerContextInternal: Send + Sync {
    async fn fire_write_internal(&mut self, msg: &mut MessageInternal);
    async fn fire_write_exception_internal(&mut self, err: Error);
    async fn fire_close_internal(&mut self);

    fn name(&self) -> &str;
    fn as_any(&mut self) -> &mut (dyn Any + Send + Sync);
    fn set_next_out_ctx(
        &mut self,
        next_out_ctx: Option<Arc<Mutex<dyn OutboundHandlerContextInternal>>>,
    );
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
    );
}
