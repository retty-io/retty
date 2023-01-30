//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

#[cfg(test)]
mod bootstrap_test;

use crate::channel::Pipeline;
use crate::transport::AsyncTransportWrite;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

mod bootstrap_tcp_client;
mod bootstrap_tcp_server;
mod bootstrap_udp_client;
mod bootstrap_udp_server;

pub use bootstrap_tcp_client::BootstrapTcpClient;
pub use bootstrap_tcp_server::BootstrapTcpServer;
pub use bootstrap_udp_client::BootstrapUdpClient;
pub use bootstrap_udp_server::BootstrapUdpServer;

/// Creates a new [Pipeline]
pub type PipelineFactoryFn<R, W> = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = Arc<Pipeline<R, W>>> + Send + 'static>>)
        + Send
        + Sync,
>;

const MAX_DURATION: u64 = 86400; // 1 day
