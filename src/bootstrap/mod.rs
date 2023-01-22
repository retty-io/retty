//! The helper classes with fluent API which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;
use crate::transport::AsyncTransportWrite;

use std::future::Future;
use std::pin::Pin;

mod bootstrap_tcp_client;
mod bootstrap_tcp_server;
mod bootstrap_udp_client;
mod bootstrap_udp_server;

pub use bootstrap_tcp_client::BootstrapTcpClient;
pub use bootstrap_tcp_server::BootstrapTcpServer;
pub use bootstrap_udp_client::BootstrapUdpClient;
pub use bootstrap_udp_server::BootstrapUdpServer;

/// Creates a new [Pipeline]
pub type PipelineFactoryFn = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = Pipeline> + Send + 'static>>)
        + Send
        + Sync,
>;

const MAX_DURATION: u64 = 86400; // 1 day
