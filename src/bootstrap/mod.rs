//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(feature = "sans-io"))]
use crate::transport::AsyncTransportWrite;
#[cfg(not(feature = "sans-io"))]
pub(crate) mod async_bootstrap;
#[cfg(not(feature = "sans-io"))]
pub use async_bootstrap::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

#[cfg(feature = "sans-io")]
use tokio::sync::broadcast::Sender;
#[cfg(feature = "sans-io")]
pub(crate) mod sansio_bootstrap;
#[cfg(feature = "sans-io")]
pub use sansio_bootstrap::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

/// Creates a new [Pipeline]
#[cfg(not(feature = "sans-io"))]
pub type PipelineFactoryFn<R, W> = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = Arc<Pipeline<R, W>>> + Send + 'static>>)
        + Send
        + Sync,
>;

/// Creates a new [Pipeline]
#[cfg(feature = "sans-io")]
pub type PipelineFactoryFn<R, W> = Box<
    dyn (Fn(Sender<R>) -> Pin<Box<dyn Future<Output = Arc<Pipeline<R, W>>> + Send + 'static>>)
        + Send
        + Sync,
>;

const MAX_DURATION: u64 = 86400; // 1 day
