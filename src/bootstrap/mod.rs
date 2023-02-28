//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;

#[cfg(not(feature = "metal-io"))]
use std::future::Future;
#[cfg(not(feature = "metal-io"))]
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(feature = "metal-io"))]
use crate::transport::AsyncTransportWrite;
#[cfg(not(feature = "metal-io"))]
pub(crate) mod async_io;
#[cfg(not(feature = "metal-io"))]
pub use self::async_io::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_ecn_client::BootstrapUdpEcnClient,
    bootstrap_udp_ecn_server::BootstrapUdpEcnServer, bootstrap_udp_server::BootstrapUdpServer,
};

#[cfg(feature = "metal-io")]
pub(crate) mod metal_io;
#[cfg(feature = "metal-io")]
pub use metal_io::{
    bootstrap_client_udp::BootstrapClientUdp, bootstrap_server_udp::BootstrapServerUdp,
};

/*bootstrap_client_tcp::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
bootstrap_udp_ecn_client::BootstrapUdpEcnClient,
bootstrap_udp_ecn_server::BootstrapUdpEcnServer*/

/// Creates a new [Pipeline]
#[cfg(not(feature = "metal-io"))]
pub type PipelineFactoryFn<R, W> = Box<
    dyn (Fn(
            Box<dyn AsyncTransportWrite + Send + Sync>,
        ) -> Pin<Box<dyn Future<Output = Arc<Pipeline<R, W>>> + Send + 'static>>)
        + Send
        + Sync,
>;

#[cfg(feature = "metal-io")]
use mio_extras::channel::Sender;
/// Creates a new [Pipeline]
#[cfg(feature = "metal-io")]
pub type PipelineFactoryFn<R, W> = Box<dyn Fn(Sender<R>) -> Arc<Pipeline<R, W>>>;

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day
