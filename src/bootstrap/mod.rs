use crate::channel::pipeline::PipelineContext;
use crate::transport::AsyncTransportWrite;

use std::future::Future;
use std::pin::Pin;

pub mod bootstrap_tcp_client;
pub mod bootstrap_tcp_server;
pub mod bootstrap_udp_client;
pub mod bootstrap_udp_server;

pub type PipelineFactoryFn = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = PipelineContext> + Send + 'static>>)
        + Send
        + Sync,
>;

const MAX_DURATION: u64 = 86400; // 1 day
