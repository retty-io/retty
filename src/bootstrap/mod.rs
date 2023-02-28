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
    bootstrap_client_tcp::BootstrapClientTcp, bootstrap_client_udp::BootstrapClientUdp,
    bootstrap_client_udp_ecn::BootstrapClientUdpEcn, bootstrap_server_tcp::BootstrapServerTcp,
    bootstrap_server_udp::BootstrapServerUdp, bootstrap_server_udp_ecn::BootstrapServerUdpEcn,
};

#[cfg(feature = "metal-io")]
pub(crate) mod metal_io;
#[cfg(feature = "metal-io")]
pub use metal_io::{
    bootstrap_client_tcp::BootstrapClientTcp,
    bootstrap_client_udp::BootstrapClientUdp,
    bootstrap_server_tcp::BootstrapServerTcp,
    bootstrap_server_udp::BootstrapServerUdp,
    //bootstrap_client_udp_ecn::BootstrapClientUdpEcn,
    //bootstrap_server_udp_ecn::BootstrapServerUdpEcn,
};

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
pub type PipelineFactoryFn<R, W> = Box<dyn (Fn(Sender<R>) -> Arc<Pipeline<R, W>>) + Send + Sync>;

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day
