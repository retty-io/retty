//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;

#[cfg(not(feature = "sync-io"))]
use std::future::Future;
#[cfg(not(feature = "sync-io"))]
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(feature = "sync-io"))]
use crate::transport::AsyncTransportWrite;
#[cfg(not(feature = "sync-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "sync-io"))]
pub use self::async_io::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_ecn_client::BootstrapUdpEcnClient,
    bootstrap_udp_ecn_server::BootstrapUdpEcnServer, bootstrap_udp_server::BootstrapUdpServer,
};

#[cfg(feature = "sync-io")]
pub(crate) mod sync_io;
#[cfg(feature = "sync-io")]
pub use sync_io::{
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

/*bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
bootstrap_udp_ecn_client::BootstrapUdpEcnClient,
bootstrap_udp_ecn_server::BootstrapUdpEcnServer*/

/// Creates a new [Pipeline]
#[cfg(not(feature = "sync-io"))]
pub type PipelineFactoryFn<R, W> = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = Arc<Pipeline<R, W>>> + Send + 'static>>)
        + Send
        + Sync,
>;

#[cfg(feature = "sync-io")]
use tokio::sync::broadcast::Sender;
/// Creates a new [Pipeline]
#[cfg(feature = "sync-io")]
pub type PipelineFactoryFn<R, W> = Box<dyn Fn(Sender<R>) -> Arc<Pipeline<R, W>>>;

const MAX_DURATION: u64 = 86400; // 1 day
const MIN_DURATION: u64 = 1; // 1 second
