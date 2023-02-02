use async_trait::async_trait;
use std::any::Any;
use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use crate::runtime::sync::Mutex;

#[doc(hidden)]
#[async_trait]
pub trait InboundHandlerInternal: Send + Sync {
    async fn transport_active_internal(&mut self, ctx: &dyn InboundContextInternal);
    async fn transport_inactive_internal(&mut self, ctx: &dyn InboundContextInternal);

    async fn read_internal(
        &mut self,
        ctx: &dyn InboundContextInternal,
        msg: Box<dyn Any + Send + Sync>,
    );
    async fn read_exception_internal(
        &mut self,
        ctx: &dyn InboundContextInternal,
        err: Box<dyn Error + Send + Sync>,
    );
    async fn read_eof_internal(&mut self, ctx: &dyn InboundContextInternal);

    async fn read_timeout_internal(&mut self, ctx: &dyn InboundContextInternal, now: Instant);
    async fn poll_timeout_internal(
        &mut self,
        ctx: &dyn InboundContextInternal,
        timeout: &mut Instant,
    );
}

#[doc(hidden)]
#[async_trait]
pub trait InboundContextInternal: Send + Sync {
    async fn fire_transport_active_internal(&self);
    async fn fire_transport_inactive_internal(&self);
    async fn fire_read_internal(&self, msg: Box<dyn Any + Send + Sync>);
    async fn fire_read_exception_internal(&self, err: Box<dyn Error + Send + Sync>);
    async fn fire_read_eof_internal(&self);
    async fn fire_read_timeout_internal(&self, now: Instant);
    async fn fire_poll_timeout_internal(&self, timeout: &mut Instant);

    fn name(&self) -> &str;
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn set_next_in_context(
        &mut self,
        next_in_context: Option<Arc<Mutex<dyn InboundContextInternal>>>,
    );
    fn set_next_in_handler(
        &mut self,
        next_in_handler: Option<Arc<Mutex<dyn InboundHandlerInternal>>>,
    );
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Arc<Mutex<dyn OutboundContextInternal>>>,
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
        ctx: &dyn OutboundContextInternal,
        msg: Box<dyn Any + Send + Sync>,
    );
    async fn write_exception_internal(
        &mut self,
        ctx: &dyn OutboundContextInternal,
        err: Box<dyn Error + Send + Sync>,
    );
    async fn close_internal(&mut self, ctx: &dyn OutboundContextInternal);
}

#[doc(hidden)]
#[async_trait]
pub trait OutboundContextInternal: Send + Sync {
    async fn fire_write_internal(&self, msg: Box<dyn Any + Send + Sync>);
    async fn fire_write_exception_internal(&self, err: Box<dyn Error + Send + Sync>);
    async fn fire_close_internal(&self);

    fn name(&self) -> &str;
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn set_next_out_context(
        &mut self,
        next_out_context: Option<Arc<Mutex<dyn OutboundContextInternal>>>,
    );
    fn set_next_out_handler(
        &mut self,
        next_out_handler: Option<Arc<Mutex<dyn OutboundHandlerInternal>>>,
    );
}
