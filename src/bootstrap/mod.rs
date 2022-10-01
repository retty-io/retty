use crate::channel::pipeline::PipelineContext;
use crate::transport::AsyncTransportWrite;

use std::future::Future;
use std::pin::Pin;

pub mod client_bootstrap_tcp;
pub mod client_bootstrap_udp;
pub mod server_bootstrap_tcp;
pub mod server_bootstrap_udp;

pub type PipelineFactoryFn = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = PipelineContext> + Send + 'static>>)
        + Send
        + Sync,
>;
